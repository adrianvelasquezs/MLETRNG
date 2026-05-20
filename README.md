# LETRNG - Implementación y Comparativa de Configuraciones de Hilos

Comparativa de configuraciones de hilos para el algoritmo LETRNG (Linear Feedback Shift Register with Random Number
Generation).

## Descripción

Este proyecto implementa varias configuraciones del algoritmo LETRNG y proporciona herramientas para:

- Ejecutar generaciones de números aleatorios
- Recopilar estadísticas de cada configuración
- Calcular media, desviación estándar y varianza (muestral) de las métricas principales

### Configuraciones disponibles

1. **Original (2W + 2S)**: Algoritmo fiel al paper original
    - 2 hilos escritores (writers)
    - 2 hilos muestreadores (samplers)
    - von Neumann sobre fold(x64)/fold(y64)

2. **Modificado 3 hilos (2W + 1S)**: Reducción a un solo sampler
    - 2 hilos escritores
    - 1 hilo muestreador
    - von Neumann sobre pares consecutivos del flujo

3. **Modificado 5 hilos (3W + 2S)**: Escritor extra
    - 3 hilos escritores
    - 2 hilos muestreadores
    - Mismo post-procesamiento que la configuración original

4. **Modificado 6 hilos (3W + 3S)**: 3 samplers con pares cíclicos
    - 3 hilos escritores
    - 3 hilos muestreadores
    - von Neumann sobre 3 pares cíclicos: (bx,by), (by,bz), (bz,bx)

## Requisitos

- **Rust**: versión 1.56 o superior
- **Python**: versión 3.7 o superior (para scripts de estadísticas)

## Compilación

Compilar la versión debug:

```bash
cargo build
```

Compilar la versión optimizada (recomendada para benchmarks):

```bash
cargo build --release
```

## Uso

### Ejecutar el programa una sola vez

```bash
cargo run --release
```

Salida:

- Tabla de estadísticas por configuración
- Métricas: entropía, fracción de bits 1, correlación serial, bytes distintos, ancho de banda (kbps)

### Ejecutar estadísticas de múltiples corridas

Para recopilar estadísticas sobre N corridas del programa, usa el script `scripts/collect_stats.py`:

```bash
python3 scripts/collect_stats.py --runs 10
```

#### Opciones disponibles

- `--runs` / `-n` (int, default: 10)  
  Número de veces que se ejecuta el binario

- `--timeout` / `-t` (float, default: 60.0)  
  Segundos máximos de espera por ejecución. Aumenta si el programa tarda mucho.

- `--retries` / `-r` (int, default: 1)  
  Número de reintentos por ejecución si hay timeout o error transitorio.

- `--continue-on-error` (flag)  
  Si se activa, el script continúa recogiendo otras corridas incluso si una falla. Los valores faltantes aparecerán
  vacíos en el CSV.

- `--binary` / `-b` (str)  
  Ruta al binario (default: `./target/release/mletrng-rust`). El script compilará automáticamente si no encuentra el
  binario.

- `--build` (flag)  
  Fuerza compilación con `cargo build --release` antes de ejecutar.

- `--csv` (str, default: `runs_raw.csv`)  
  Nombre del fichero CSV de salida con valores crudos por ejecución.

#### Ejemplos de uso

**Ejecutar 10 corridas con timeout de 120 segundos:**

```bash
python3 scripts/collect_stats.py --runs 10 --timeout 120
```

**Ejecutar 20 corridas, continuar si hay errores:**

```bash
python3 scripts/collect_stats.py --runs 20 --continue-on-error
```

**Ejecutar 50 corridas con reintentos y salida personalizada:**

```bash
python3 scripts/collect_stats.py --runs 50 --retries 2 --csv results_50runs.csv
```

**Forzar compilación antes de ejecutar 10 corridas:**

```bash
python3 scripts/collect_stats.py --build --runs 10
```

### Salida del script de estadísticas

El script `collect_stats.py` realiza N ejecuciones del binario y genera:

1. **Resumen en pantalla (stdout)**  
   Tabla con **media**, **desviación estándar** y **varianza** (muestral, ddof=1) para cada métrica y configuración.

   Ejemplo:
   ```
   Resumen (media, varianza, desviación estándar muestral) por configuración
   
   --- Original  (2W + 2S) ---
   entropy     : mean = 7.10539, std = 0.154952, var = 0.0240102  (n=10)
   bit_ratio   : mean = 0.51314, std = 0.0637654, var = 0.00406602  (n=10)
   ...
   ```

2. **CSV con valores crudos**: `runs_raw.csv` (o el nombre especificado con `--csv`)
    - Columnas: `run`, `config`, `entropy`, `bit_ratio`, `serial_corr`, `distinct`, `kbps`
    - Una fila por ejecución y por configuración
    - Útil para análisis posteriores, gráficos, estadísticas adicionales

3. **CSV con resumen estadístico**: `runs_raw_summary.csv` (o `<csv_name>_summary.csv`)
    - Columnas: `config`, `metric`, `count`, `mean`, `std`, `variance`
    - Una fila por combinación de configuración y métrica
    - Resumen consolidado de todos los valores calculados

#### Interpretación de métricas

- **entropy** (bits/B): Entropía de Shannon (rango: 0-8)
    - Mayor valor = mejor aleatoriedad
    - Ideal: >= 7.5 para datos criptográficos

- **bit_ratio**: Fracción de bits a 1 en toda la secuencia generada (rango: 0-1)
    - Ideal: 0.5 (igual número de 0s y 1s)
    - Desviaciones significativas indican sesgo en los datos

- **serial_corr**: Correlación serial entre bytes consecutivos (rango: -1 a 1)
    - Ideal: cercano a 0 (débil autocorrelación)
    - Valores altos indican dependencia entre muestras consecutivas

- **distinct**: Número de bytes distintos observados (rango: 0-256)
    - Ideal: 256 (todos los valores posibles representados)
    - Valores menores indican falta de cobertura del espacio de bytes

- **kbps**: Ancho de banda en kilobits por segundo
    - Métrica de velocidad: bytes generados por unidad de tiempo
    - A mayor valor, mayor velocidad de generación
    - Trade-off: velocidad vs. calidad criptográfica

## Ejemplo de ejecución completa

### Paso 1: Compilación

```bash
cd /ruta/a/mletrng-rust
cargo build --release
```

### Paso 2: Ejecución única (verificación)

```bash
cargo run --release
```

### Paso 3: Recopilar estadísticas (10 corridas)

```bash
python3 scripts/collect_stats.py --runs 10 --timeout 120
```

Después de ejecutar, tendrás:

- En pantalla: resumen con media, std, varianza de cada métrica por configuración (n=10)
- En `runs_raw.csv`: todos los valores crudos (40 filas: 10 runs × 4 configs)
- En `runs_raw_summary.csv`: estadísticas finales resumidas (20 filas: 4 configs × 5 metrics)

## Notas sobre estadísticas

### Varianza y desviación estándar (Sample vs. Population)

- El script usa **varianza y desviación estándar muestral** (denominador n-1, ddof=1)
- Se calcula mediante `statistics.variance()` y `statistics.stdev()` en Python
- Apropiado cuando se recopilan pocas muestras (n < 30) del proceso
- Si necesitas varianza **poblacional** (denominador n), puedes modificar el script para usar `statistics.pvariance()`

### Recomendaciones de muestreo

- **n ≥ 10**: Mínimo recomendado para tener estimaciones básicas
- **n ≥ 30**: Recomendado para estimaciones más estables (de acuerdo al Teorema del Límite Central)
- **n ≥ 100**: Para estimaciones de muy alta confianza

Con pocos datos (n < 10), las estimaciones de varianza pueden tener alta incertidumbre.

### Cálculo de métricas

El script calcula para cada métrica (entropy, bit_ratio, serial_corr, distinct, kbps):

- **count**: Número de valores válidos recopilados
- **mean**: Promedio aritmético (μ)
- **std**: Desviación estándar muestral (σ, ddof=1)
- **variance**: Varianza muestral (σ², ddof=1)

## Troubleshooting

### El programa tarda mucho o se cuelga

**Problema**: El script reporta "Run timed out" o tarda más de lo esperado.

**Soluciones**:

- Aumenta el timeout:
  ```bash
  python3 scripts/collect_stats.py --runs 10 --timeout 300
  ```
- Usa la opción `--continue-on-error` para no detener si una corrida falla:
  ```bash
  python3 scripts/collect_stats.py --runs 10 --timeout 120 --continue-on-error
  ```
- Compila en release (optimizado):
  ```bash
  cargo build --release
  ```

### El script reporta "no configurations parsed"

**Problema**: El script no puede parsear la salida del binario.

**Soluciones**:

1. Verifica que el binario se ejecuta y produce salida:
   ```bash
   ./target/release/mletrng-rust
   ```
2. Si el programa no se ejecuta, compila nuevamente:
   ```bash
   cargo build --release
   ```
3. Fuerza compilación usando el flag `--build`:
   ```bash
   python3 scripts/collect_stats.py --build --runs 1
   ```

### El binario no se genera

**Problema**: `cargo build --release` falla.

**Soluciones**:

1. Verifica que Rust está instalado:
   ```bash
   rustc --version
   cargo --version
   ```
2. Actualiza Rust:
   ```bash
   rustup update
   ```
3. Limpia y recompila:
   ```bash
   cargo clean
   cargo build --release
   ```

### Los valores de una métrica están sin cambios o siempre son iguales

**Problema**: Una métrica (ej: `bit_ratio`, `distinct`) tiene varianza cero.

**Posibles causas**:

- Es el comportamiento esperado de algunas configuraciones (ej: 6 hilos siempre genera `bit_ratio = 0.5`)
- Muestra de tamaño muy pequeño (n < 3)

**Solución**: Ejecuta con más corridas para obtener mejor estadística:

```bash
python3 scripts/collect_stats.py --runs 50
```

## Modelo teórico

LETRNG es un generador de números aleatorios que utiliza:

- **Retroalimentación lineal**: Registros de desplazamiento con retroalimentación lineal (LFSR)
- **Von Neumann extractor**: Post-procesamiento para mejorar la distribución
- **Muestreo multihilo**: Reduce correlaciones mediante múltiples caminos de entrada/salida

Las diferentes configuraciones experimentan con:

- Número de hilos escritores (productores de estado): 2 vs. 3
- Número de hilos muestreadores (extractores): 1 vs. 2 vs. 3
- Forma de aplicar Von Neumann: sobre pares específicos vs. pares cíclicos

