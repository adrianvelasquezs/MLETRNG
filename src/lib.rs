/// Definición de la función de generación de números aleatorios basada en el algoritmo LETRNG
/// de Chen et al. 2023, con varias configuraciones de hilos para comparar su impacto

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;

// ── Constantes ───────────────────────────────────────────────────────────────
/// Iteraciones base de los hilos escritores por ronda.
const N: u64 = 20_000;

// ============================================================================
// Utilidad compartida
// ============================================================================

/// XOR-fold de 64 bits → 1 bit (Algorithm 4 de Chen et al. 2023).
/// Equivale a la paridad de los bits del argumento.
pub fn fold_xor(b: u64) -> u8 {
    (b.count_ones() % 2) as u8
}

/// Macro para crear un hilo escritor ascendente que controla `active`.
macro_rules! writer_ascending_ctrl {
    ($coin:expr, $active:expr, $iters:expr) => {{
        let (c, a) = (Arc::clone(&$coin), Arc::clone(&$active));
        thread::spawn(move || {
            for n in 0u64..$iters { c.store((n % 2) as u8, Ordering::Relaxed); }
            a.store(false, Ordering::Release);
        })
    }};
}

/// Macro para crear un hilo escritor descendente (sin control de `active`).
macro_rules! writer_descending {
    ($coin:expr, $iters:expr) => {{
        let c = Arc::clone(&$coin);
        thread::spawn(move || {
            for n in (0u64..$iters).rev() { c.store((n % 2) as u8, Ordering::Relaxed); }
        })
    }};
}

/// Macro para crear un hilo escritor ascendente adicional (sin control).
macro_rules! writer_ascending_extra {
    ($coin:expr, $iters:expr) => {{
        let c = Arc::clone(&$coin);
        thread::spawn(move || {
            for n in 0u64..$iters { c.store((n % 2) as u8, Ordering::Relaxed); }
        })
    }};
}

/// Macro para crear un hilo muestreador que acumula una ventana de 64 bits.
macro_rules! sampler {
    ($coin:expr, $active:expr) => {{
        let (c, a) = (Arc::clone(&$coin), Arc::clone(&$active));
        thread::spawn(move || {
            let mut acc: u64 = 0;
            while a.load(Ordering::Acquire) {
                acc = (acc << 1) | (c.load(Ordering::Relaxed) as u64 & 1);
            }
            acc
        })
    }};
}

// ============================================================================
// Config 1 — LETRNG Original: 4 hilos (2 escritores + 2 samplers)
// ============================================================================
//
// Algoritmo fiel a Chen et al. 2023, Sección 4.
// Por ronda: XOR-fold(x64) y XOR-fold(y64) → von Neumann → 0 ó 1 bit.
// ============================================================================

pub struct Letrng;

impl Letrng {
    pub fn new() -> Self { Letrng }

    pub fn generate_bytes(&self, count: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(count);
        while out.len() < count {
            for b in self.generate_u64().to_le_bytes() {
                if out.len() < count { out.push(b); }
            }
        }
        out
    }

    pub fn generate_u64(&self) -> u64 {
        let mut pool: u64 = 0;
        let mut bits: u32 = 0;
        while bits < 64 {
            let (x, y) = self.run_round();
            let bx = fold_xor(x);
            let by = fold_xor(y);
            if bx != by {
                pool ^= bx as u64;
                pool = pool.rotate_left(1);
                bits += 1;
            }
        }
        pool
    }

    fn run_round(&self) -> (u64, u64) {
        let coin   = Arc::new(AtomicU8::new(0));
        let active = Arc::new(AtomicBool::new(true));

        let t1 = writer_ascending_ctrl!(coin, active, N);
        let t2 = writer_descending!(coin, N);
        let tx = sampler!(coin, active);
        let ty = sampler!(coin, active);

        t1.join().unwrap();
        let x = tx.join().unwrap();
        let y = ty.join().unwrap();
        t2.join().unwrap();
        (x, y)
    }
}

impl Default for Letrng { fn default() -> Self { Self::new() } }

// ============================================================================
// Config 2 — LetrngThreeThread: 3 hilos (2 escritores + 1 sampler)
// ============================================================================
//
// Modificación: se elimina un hilo muestreador. Von Neumann se aplica sobre
// pares consecutivos del flujo bruto del único sampler.
// Hipótesis: si el flujo bruto tiene autocorrelación temporal, von Neumann
// no puede corregirla (solo corrige sesgo, no dependencia serial).
// ============================================================================

pub struct LetrngThreeThread;

impl LetrngThreeThread {
    pub fn new() -> Self { LetrngThreeThread }

    pub fn generate_bytes(&self, count: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(count);
        while out.len() < count {
            for b in self.generate_u64().to_le_bytes() {
                if out.len() < count { out.push(b); }
            }
        }
        out
    }

    pub fn generate_u64(&self) -> u64 {
        let mut pool: u64 = 0;
        let mut bits: u32 = 0;
        while bits < 64 {
            let raw = self.run_single_sampler_round();
            let mut i = 0;
            while i + 1 < raw.len() && bits < 64 {
                let (b0, b1) = (raw[i], raw[i + 1]);
                i += 2;
                if b0 != b1 {
                    pool ^= b0 as u64;
                    pool = pool.rotate_left(1);
                    bits += 1;
                }
            }
        }
        pool
    }

    fn run_single_sampler_round(&self) -> Vec<u8> {
        // Usa N*4 para producir suficientes bits tras el descarte de Von Neumann.
        let iters = N * 4;
        let coin   = Arc::new(AtomicU8::new(0));
        let active = Arc::new(AtomicBool::new(true));

        let t1 = writer_ascending_ctrl!(coin, active, iters);
        let t2 = writer_descending!(coin, iters);

        let (cs, ac) = (Arc::clone(&coin), Arc::clone(&active));
        let ts = thread::spawn(move || {
            let mut raw: Vec<u8> = Vec::with_capacity(512);
            while ac.load(Ordering::Acquire) {
                raw.push(cs.load(Ordering::Relaxed) & 1);
            }
            raw
        });

        t1.join().unwrap();
        let raw = ts.join().unwrap();
        t2.join().unwrap();
        raw
    }
}

impl Default for LetrngThreeThread { fn default() -> Self { Self::new() } }

// ============================================================================
// Config 3 — LetrngFiveThread: 5 hilos (3 escritores + 2 samplers)
// ============================================================================
//
// Se añade un tercer hilo escritor ascendente (con N diferente) que incrementa
// la contención sobre `coin`. La hipótesis es que más hilos escribiendo
// simultáneamente producen mayor no-determinismo en la variable compartida y,
// por tanto, más entropía por ronda.
// El post-procesamiento es idéntico al original: XOR-fold + von Neumann.
// ============================================================================

pub struct LetrngFiveThread;

impl LetrngFiveThread {
    pub fn new() -> Self { LetrngFiveThread }

    pub fn generate_bytes(&self, count: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(count);
        while out.len() < count {
            for b in self.generate_u64().to_le_bytes() {
                if out.len() < count { out.push(b); }
            }
        }
        out
    }

    pub fn generate_u64(&self) -> u64 {
        let mut pool: u64 = 0;
        let mut bits: u32 = 0;
        while bits < 64 {
            let (x, y) = self.run_round();
            let bx = fold_xor(x);
            let by = fold_xor(y);
            if bx != by {
                pool ^= bx as u64;
                pool = pool.rotate_left(1);
                bits += 1;
            }
        }
        pool
    }

    fn run_round(&self) -> (u64, u64) {
        let coin   = Arc::new(AtomicU8::new(0));
        let active = Arc::new(AtomicBool::new(true));

        // Writer 1 (↑, N iters) — controla active
        let t1 = writer_ascending_ctrl!(coin, active, N);
        // Writer 2 (↓, N iters)
        let t2 = writer_descending!(coin, N);
        // Writer 3 (↑, N*2/3 iters) — extra escritor con longitud diferente
        // Crea una tercera fuente de contención con duración distinta a T1/T2.
        let t3 = writer_ascending_extra!(coin, N * 2 / 3);

        let tx = sampler!(coin, active);
        let ty = sampler!(coin, active);

        t1.join().unwrap();
        let x = tx.join().unwrap();
        let y = ty.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();
        (x, y)
    }
}

impl Default for LetrngFiveThread { fn default() -> Self { Self::new() } }

// ============================================================================
// Config 4 — LetrngSixThread: 6 hilos (3 escritores + 3 samplers)
// ============================================================================
//
// Extensión completa: 3 escritores (mayor contención) + 3 samplers (X, Y, Z).
// XOR-fold de cada acumulador: bx, by, bz.
// Von Neumann sobre los 3 pares cíclicos: (bx,by), (by,bz), (bz,bx).
// Cada par válido aporta 1 bit al pool → hasta 3 bits por ronda.
// Esto combina mayor entropía de escritura con mayor tasa de extracción.
//
// Nota sobre independencia: los pares comparten bits (bx aparece en dos pares),
// por lo que los 3 bits por ronda no son completamente independientes entre sí.
// Sin embargo, cada par individual produce un bit individualmente uniforme bajo
// la suposición de independencia entre samplers.
// ============================================================================

pub struct LetrngSixThread;

impl LetrngSixThread {
    pub fn new() -> Self { LetrngSixThread }

    pub fn generate_bytes(&self, count: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(count);
        while out.len() < count {
            for b in self.generate_u64().to_le_bytes() {
                if out.len() < count { out.push(b); }
            }
        }
        out
    }

    pub fn generate_u64(&self) -> u64 {
        let mut pool: u64 = 0;
        let mut bits: u32 = 0;
        while bits < 64 {
            let (x, y, z) = self.run_round();
            let bx = fold_xor(x);
            let by = fold_xor(y);
            let bz = fold_xor(z);

            // Von Neumann sobre 3 pares cíclicos: (bx,by), (by,bz), (bz,bx)
            for (a, b) in [(bx, by), (by, bz), (bz, bx)] {
                if bits < 64 && a != b {
                    pool ^= a as u64;
                    pool = pool.rotate_left(1);
                    bits += 1;
                }
            }
        }
        pool
    }

    fn run_round(&self) -> (u64, u64, u64) {
        let coin   = Arc::new(AtomicU8::new(0));
        let active = Arc::new(AtomicBool::new(true));

        // 3 escritores
        let t1 = writer_ascending_ctrl!(coin, active, N);
        let t2 = writer_descending!(coin, N);
        let t3 = writer_ascending_extra!(coin, N * 2 / 3);

        // 3 samplers independientes
        let tx = sampler!(coin, active);
        let ty = sampler!(coin, active);
        let tz = sampler!(coin, active);

        t1.join().unwrap();
        let x = tx.join().unwrap();
        let y = ty.join().unwrap();
        let z = tz.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();
        (x, y, z)
    }
}

impl Default for LetrngSixThread { fn default() -> Self { Self::new() } }

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn check(bytes: Vec<u8>, n: usize) {
        assert_eq!(bytes.len(), n);
        assert!(bytes.iter().any(|&b| b != 0));
        assert!(bytes.iter().any(|&b| b != 0xFF));
    }

    #[test] fn original_ok()    { check(Letrng::new().generate_bytes(8), 8); }
    #[test] fn modified_ok()    { check(LetrngThreeThread::new().generate_bytes(8), 8); }
    #[test] fn five_thread_ok() { check(LetrngFiveThread::new().generate_bytes(8), 8); }
    #[test] fn six_thread_ok()  { check(LetrngSixThread::new().generate_bytes(8), 8); }
}
