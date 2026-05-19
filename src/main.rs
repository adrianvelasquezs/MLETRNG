/// Comparativa de configuraciones de hilos para el algoritmo LETRNG.

use mletrng_rust::{Letrng, LetrngFiveThread, LetrngThreeThread, LetrngSixThread};
use std::time::{Duration, Instant};

fn main() {
    println!("=================================================================");
    println!("   LETRNG - Comparativa de configuraciones de hilos");
    println!("=================================================================\n");

    let n_bytes = 256;

    // Ejecutar y medir cada configuración
    let configs: &[(&str, Box<dyn Fn() -> Vec<u8>>)] = &[
        ("Original  (2W + 2S)", Box::new(|| Letrng::new().generate_bytes(n_bytes))),
        ("Modificado(2W + 1S)", Box::new(|| LetrngThreeThread::new().generate_bytes(n_bytes))),
        ("5 hilos   (3W + 2S)", Box::new(|| LetrngFiveThread::new().generate_bytes(n_bytes))),
        ("6 hilos   (3W + 3S)", Box::new(|| LetrngSixThread::new().generate_bytes(n_bytes))),
    ];

    let mut resultados: Vec<(String, Stats, Duration)> = Vec::new();

    for (nombre, generador) in configs {
        println!(">> {}", nombre);
        let t0 = Instant::now();
        let bytes = generador();
        let elapsed = t0.elapsed();
        let stats = Stats::compute(&bytes);
        stats.print_inline(elapsed, n_bytes);
        println!();
        resultados.push((nombre.to_string(), stats, elapsed));
    }

    // Tabla comparativa final
    println!("┌──────────────────────┬──────────┬──────────┬──────────┬──────────┐");
    println!("│ Métrica              │ Orig 4H  │ Mod  3H  │ Mod 5H   │ Mod 6 H  │");
    println!("├──────────────────────┼──────────┼──────────┼──────────┼──────────┤");

    // Entropía
    print!("│ Entropía (bits/B)    │");
    for (_, s, _) in &resultados { print!(" {:>7.4}  │", s.entropy); }
    println!();

    // Fracción bits 1
    print!("│ Frac. bits '1'       │");
    for (_, s, _) in &resultados { print!(" {:>7.4}  │", s.bit_ratio); }
    println!();

    // Correlación serial
    print!("│ Corr. serial         │");
    for (_, s, _) in &resultados { print!(" {:>+7.4}  │", s.serial_corr); }
    println!();

    // Bytes distintos
    print!("│ Bytes distintos/256  │");
    for (_, s, _) in &resultados { print!(" {:>7}  │", s.distinct); }
    println!();

    // kbps
    print!("│ Ancho de banda (kbps)│");
    for (_, s, elapsed) in &resultados {
        print!(" {:>7.2}  │", s.kbps(*elapsed, n_bytes));
    }
    println!();

    println!("└──────────────────────┴──────────┴──────────┴──────────┴──────────┘");

    println!("\nLeyenda de configuraciones:");
    println!("  W = hilo escritor (writer)  |  S = hilo muestreador (sampler)");
    println!("  Original  (2W+2S): algoritmo fiel al paper, von Neumann sobre fold(x64)/fold(y64)");
    println!("  Mod 3H    (2W+1S): un solo sampler, von Neumann sobre pares consecutivos del flujo");
    println!("  Mod 5H    (3W+2S): escritor extra, mismos 2 samplers, mismo post-procesamiento");
    println!("  Mod 6H    (3W+3S): 3 samplers, von Neumann sobre 3 pares cíclicos (bx,by),(by,bz),(bz,bx)");
}

// ── Estadísticas ─────────────────────────────────────────────────────────────

struct Stats {
    entropy:     f64,
    bit_ratio:   f64,
    serial_corr: f64,
    distinct:    usize,
    hex_sample:  String,
}

impl Stats {
    fn compute(data: &[u8]) -> Self {
        let n = data.len();
        let ones: u32 = data.iter().map(|b| b.count_ones()).sum();
        let bit_ratio = ones as f64 / (n * 8) as f64;

        let mut freq = [0u64; 256];
        for &b in data { freq[b as usize] += 1; }

        let entropy: f64 = freq.iter().filter(|&&c| c > 0).map(|&c| {
            let p = c as f64 / n as f64;
            -p * p.log2()
        }).sum();

        let distinct = freq.iter().filter(|&&c| c > 0).count();

        let serial_corr = {
            let nf = n as f64;
            let sum: f64      = data.iter().map(|&b| b as f64).sum();
            let sum_sq: f64   = data.iter().map(|&b| (b as f64).powi(2)).sum();
            let sum_prod: f64 = data.windows(2).map(|w| w[0] as f64 * w[1] as f64).sum();
            let num = nf * sum_prod - sum * sum;
            let den = (nf * sum_sq - sum * sum).powi(2);
            if den <= 0.0 { 0.0 } else { num / den.sqrt() }
        };

        let hex_sample = data.iter().take(24).map(|b| format!("{:02x}", b)).collect();

        Stats { entropy, bit_ratio, serial_corr, distinct, hex_sample }
    }

    fn kbps(&self, elapsed: Duration, n_bytes: usize) -> f64 {
        (n_bytes * 8) as f64 / elapsed.as_secs_f64() / 1000.0
    }

    fn print_inline(&self, elapsed: Duration, n_bytes: usize) {
        println!("   Entropía : {:.4} bits/B  |  frac_1 = {:.4}  |  corr = {:+.4}",
                 self.entropy, self.bit_ratio, self.serial_corr);
        println!("   Distintos: {}/256  |  kbps = {:.2}  |  t = {:.3?}",
                 self.distinct, self.kbps(elapsed, n_bytes), elapsed);
        println!("   Hex(24B) : {}", self.hex_sample);
    }
}
