#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
scripts/collect_stats.py

Ejecuta el binario varias veces, parsea su salida y calcula media y varianza
(muestral) de las métricas principales por configuración.

Uso:
  python3 scripts/collect_stats.py --runs 10

Nota: por defecto asume que el binario está en ./target/release/mletrng-rust.
Si no existe, el script intentará compilar con `cargo build --release` antes de
comenzar.

Salida:
 - imprime una tabla resumen con media y varianza (muestral, ddof=1)
 - escribe `runs_raw.csv` con los valores crudos por ejecución y por algoritmo

"""

from __future__ import annotations
import argparse
import os
import re
import subprocess
import sys
import csv
import time
from statistics import mean, variance, stdev
from typing import List, Dict, Any
from typing import List, Dict, Any

# --- Configuración por defecto
DEFAULT_BINARY = os.path.abspath("./target/release/mletrng-rust")

# --- Parsers
RE_NAME = re.compile(r"^>>\s*(.+)$")
RE_ENTROPY = re.compile(r"Entropía\s*:\s*([0-9]+\.?[0-9]*)\s*bits/B\s*\|\s*frac_1\s*=\s*([0-9]+\.?[0-9]*)\s*\|\s*corr\s*=\s*([+\-]?[0-9]+\.?[0-9]*)")
RE_DIST = re.compile(r"Distintos\s*:\s*(\d+)/256\s*\|\s*kbps\s*=\s*([0-9]+\.?[0-9]*)\s*\|\s*t\s*=\s*(\S+)")

METRICS = ["entropy", "bit_ratio", "serial_corr", "distinct", "kbps"]


def ensure_binary(binary_path: str) -> str:
    """Ensure the binary exists; if not, try to build release and return path."""
    if os.path.exists(binary_path) and os.access(binary_path, os.X_OK):
        return binary_path
    print(f"Binary {binary_path} not found or not executable. Building release binary...")
    res = subprocess.run(["cargo", "build", "--release"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if res.returncode != 0:
        print("cargo build failed:", res.stderr.decode(errors='replace'))
        sys.exit(1)
    if not os.path.exists(binary_path):
        print("Build completed but binary still not found at:", binary_path)
        sys.exit(1)
    return binary_path


def parse_run_output(output: str) -> List[Dict[str, Any]]:
    """Parse one run's stdout and return a list of metric dicts in the order
    the program prints the configurations.
    Each dict has keys: name, entropy, bit_ratio, serial_corr, distinct, kbps
    """
    lines = [l.strip() for l in output.splitlines()]
    i = 0
    results: List[Dict[str, Any]] = []
    while i < len(lines):
        mname = RE_NAME.match(lines[i])
        if mname:
            name = mname.group(1).strip()
            # next meaningful lines expected to contain entropy line and distinct line
            ent = None
            dist = None
            if i + 1 < len(lines):
                ent = RE_ENTROPY.search(lines[i+1])
            if i + 2 < len(lines):
                dist = RE_DIST.search(lines[i+2])
            # fallback: try to find the next lines containing the tokens
            if not ent:
                # scan ahead a few lines
                for j in range(i+1, min(i+6, len(lines))):
                    ent = RE_ENTROPY.search(lines[j])
                    if ent:
                        break
            if not dist:
                for j in range(i+1, min(i+6, len(lines))):
                    dist = RE_DIST.search(lines[j])
                    if dist:
                        break

            if ent and dist:
                entropy = float(ent.group(1))
                bit_ratio = float(ent.group(2))
                serial_corr = float(ent.group(3))
                distinct = int(dist.group(1))
                kbps = float(dist.group(2))
                results.append({
                    "name": name,
                    "entropy": entropy,
                    "bit_ratio": bit_ratio,
                    "serial_corr": serial_corr,
                    "distinct": distinct,
                    "kbps": kbps,
                })
                # advance index
                i += 3
                continue
            else:
                # no match; advance
                i += 1
        else:
            i += 1
    return results


def aggregate_runs(all_runs: List[List[Dict[str, Any]]]) -> Dict[str, Dict[str, List[float]]]:
    """From parsed runs (list per run of configs), build dict per algorithm name
    with lists of metric values.
    Returns: { name: { metric: [v1,v2,...] } }
    """
    agg: Dict[str, Dict[str, List[float]]] = {}
    for run_idx, run in enumerate(all_runs):
        for cfg in run:
            name = cfg["name"]
            if name not in agg:
                agg[name] = {m: [] for m in METRICS}
            for m in METRICS:
                agg[name][m].append(cfg[m])
    return agg


def print_summary(agg: Dict[str, Dict[str, List[float]]], runs: int) -> None:
    print("\nResumen (media, varianza, desviación estándar muestral) por configuración")
    print("Nota: varianza y desv. est. muestral (statistics) usan denominador n-1 (ddof=1).")
    for name, metrics in agg.items():
        print("\n--- {} ---".format(name))
        for m, vals in metrics.items():
            if len(vals) == 0:
                continue
            m_mean = mean(vals)
            m_var = variance(vals) if len(vals) > 1 else 0.0
            m_std = stdev(vals) if len(vals) > 1 else 0.0
            print(f"{m:12s}: mean = {m_mean:.6g}, std = {m_std:.6g}, var = {m_var:.6g}  (n={len(vals)})")


def write_csv(agg: Dict[str, Dict[str, List[float]]], runs: int, path: str = "runs_raw.csv") -> None:
    """Write a CSV with columns: run, config, entropy, bit_ratio, serial_corr, distinct, kbps"""
    # Build rows per run index
    configs = sorted(agg.keys())
    # transpose: obtain per-run values for each config
    rows = []
    for run_idx in range(runs):
        for cfg in configs:
            vals = {m: (agg[cfg][m][run_idx] if run_idx < len(agg[cfg][m]) else "") for m in METRICS}
            rows.append({"run": run_idx+1, "config": cfg, **vals})
    with open(path, "w", newline='') as f:
        w = csv.DictWriter(f, fieldnames=["run", "config"] + METRICS)
        w.writeheader()
        for r in rows:
            w.writerow(r)
    print(f"Raw values written to {path}")


def write_summary_csv(agg: Dict[str, Dict[str, List[float]]], summary_path: str = "runs_summary.csv") -> None:
    """Write a CSV with summary statistics per config and metric: mean, std, variance"""
    rows = []
    configs = sorted(agg.keys())
    for cfg in configs:
        for m in METRICS:
            vals = agg[cfg][m]
            if len(vals) == 0:
                continue
            m_mean = mean(vals)
            m_std = stdev(vals) if len(vals) > 1 else 0.0
            m_var = variance(vals) if len(vals) > 1 else 0.0
            rows.append({
                "config": cfg,
                "metric": m,
                "count": len(vals),
                "mean": m_mean,
                "std": m_std,
                "variance": m_var,
            })
    with open(summary_path, "w", newline='') as f:
        w = csv.DictWriter(f, fieldnames=["config", "metric", "count", "mean", "std", "variance"])
        w.writeheader()
        for r in rows:
            w.writerow(r)
    print(f"Summary statistics written to {summary_path}")


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--runs', '-n', type=int, default=10, help='Número de ejecuciones (default: 10)')
    p.add_argument('--binary', '-b', default=DEFAULT_BINARY, help='Ruta al binario (default: ./target/release/mletrng-rust)')
    p.add_argument('--build', action='store_true', help='Forzar `cargo build --release` antes de ejecutar')
    p.add_argument('--csv', default='runs_raw.csv', help='Fichero CSV de salida con valores crudos')
    p.add_argument('--timeout', '-t', type=float, default=60.0, help='Segundos antes de timeout por ejecución (default: 60)')
    p.add_argument('--retries', '-r', type=int, default=1, help='Número de reintentos por ejecución si hay timeout/error (default: 1)')
    p.add_argument('--continue-on-error', action='store_true', help='Si se activa, el script continuará recogiendo otras corridas cuando una falle (los valores faltantes se dejarán vacíos en el CSV)')
    args = p.parse_args()

    bin_path = os.path.abspath(args.binary)
    if args.build:
        print("Building release binary...")
        subprocess.run(["cargo", "build", "--release"], check=True)
    bin_path = ensure_binary(bin_path)

    all_runs = []
    print('\n=== Testing Modified LETRNG ===\n')
    for i in range(args.runs):
        attempt = 0
        success = False
        last_err_text = None
        while attempt <= args.retries and not success:
            attempt += 1
            print(f">> Running {bin_path} (run {i+1}/{args.runs}), attempt {attempt}/{args.retries+1}...")
            try:
                out = subprocess.check_output([bin_path], stderr=subprocess.STDOUT, timeout=args.timeout)
                text = out.decode(errors='replace')
                parsed = parse_run_output(text)
                if not parsed:
                    last_err_text = text
                    raise RuntimeError("no configurations parsed from run output")
                all_runs.append(parsed)
                success = True
            except subprocess.CalledProcessError as e:
                last_err_text = e.output.decode(errors='replace')
                print("Binary returned non-zero exit code; stdout/stderr:")
                print(last_err_text)
                # don't retry on non-zero exit; break
                break
            except subprocess.TimeoutExpired:
                print("Run timed out (timeout={}s)".format(args.timeout))
                if attempt <= args.retries:
                    print("Retrying...")
                    time.sleep(0.1)
                    continue
                else:
                    print("Exceeded retries for this run.")
                    break
            except Exception as e:
                print(f"Run failed: {e}")
                if last_err_text:
                    print(last_err_text)
                if attempt <= args.retries:
                    print("Retrying...")
                    time.sleep(0.1)
                    continue
                else:
                    break

        if not success:
            if args.continue_on_error:
                print(f"Run {i+1} failed after {attempt} attempts, continuing because --continue-on-error is set.")
                # represent a missing run as an empty list so aggregation will leave blanks
                all_runs.append([])
                continue
            else:
                print("Aborting due to failed run. Use --continue-on-error to continue despite failures.")
                sys.exit(1)

    # Now aggregate
    agg = aggregate_runs(all_runs)
    print_summary(agg, args.runs)
    write_csv(agg, args.runs, args.csv)
    write_summary_csv(agg, args.csv.replace('.csv', '_summary.csv'))


if __name__ == '__main__':
    main()

