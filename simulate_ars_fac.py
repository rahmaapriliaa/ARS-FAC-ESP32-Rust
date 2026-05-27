#!/usr/bin/env python3
"""
simulate_ars_fac.py
════════════════════════════════════════════════════════════════════════════════
Simulasi software ARS-FAC (Adaptive Risk-Based Sampling & Fail-Safe Actuator)
Menghasilkan  simulation_data.csv  yang identik dengan output UART ESP32-S3.
Jalankan sebelum GNUPlot:
    python simulate_ars_fac.py
    gnuplot plot_ars_fac.gp
════════════════════════════════════════════════════════════════════════════════
"""

import csv
import math
import random
import sys

# ── Konfigurasi ───────────────────────────────────────────────────────────────
OUTPUT_CSV   = "simulation_data.csv"
TOTAL_TICKS  = 150
RANDOM_SEED  = 42          # reproduksibel

STUCK_LIMIT  = 10          # siklus sama → sensor stuck
JUMP_LIMIT   = 1000        # delta ADC max per siklus
DANGER_LIMIT = 3           # siklus danger berturut → sustained

random.seed(RANDOM_SEED)

# ═════════════════════════════════════════════════════════════════════════════
# STATE
# ═════════════════════════════════════════════════════════════════════════════
class SystemState:
    def __init__(self):
        self.tick         = 0
        self.adc_raw      = 0
        self.risk_value   = 0.0
        self.mode         = "NORMAL"
        self.sampling_ms  = 2000
        self.fan_pwm      = 20
        self.led          = "HIJAU"
        self.buzzer       = "OFF"
        self.error        = "OK"
        self.prev_adc     = 0
        self.stuck_count  = 0
        self.danger_count = 0

# ═════════════════════════════════════════════════════════════════════════════
# SENSOR VALIDATION  (identik dengan Rust: validate_sensor)
# ═════════════════════════════════════════════════════════════════════════════
def validate_sensor(state: SystemState, raw: int) -> bool:
    # F1 — range check
    if raw <= 5 or raw >= 4090:
        state.error = "ERR_SENSOR_INVALID"
        return False

    # F2 — stuck check
    if raw == state.prev_adc:
        state.stuck_count += 1
        if state.stuck_count > STUCK_LIMIT:
            state.error = "ERR_SENSOR_STUCK"
            return False
    else:
        state.stuck_count = 0

    # F3 — extreme jump
    if state.prev_adc > 0:
        delta = abs(raw - state.prev_adc)
        if delta > JUMP_LIMIT:
            state.error = "ERR_EXTREME_JUMP"
            return False

    state.error = "OK"
    return True

# ═════════════════════════════════════════════════════════════════════════════
# RISK CLASSIFICATION  (identik dengan Rust: classify_risk)
# ═════════════════════════════════════════════════════════════════════════════
def classify_risk(state: SystemState):
    r = state.risk_value

    if r >= 70.0:
        state.danger_count += 1
        state.mode         = "DANGER"
        state.sampling_ms  = 500
        state.fan_pwm      = 100
        state.led          = "MERAH"
        state.buzzer       = "ON"
        if state.danger_count >= DANGER_LIMIT:
            state.error = "ERR_DANGER_SUSTAINED"
    elif r >= 40.0:
        state.danger_count = 0
        state.mode         = "WARNING"
        state.sampling_ms  = 1000
        state.fan_pwm      = 60
        state.led          = "KUNING"
        state.buzzer       = "OFF"
        state.error        = "OK"
    else:
        state.danger_count = 0
        state.mode         = "NORMAL"
        state.sampling_ms  = 2000
        state.fan_pwm      = 20
        state.led          = "HIJAU"
        state.buzzer       = "OFF"
        state.error        = "OK"

def set_fault(state: SystemState):
    state.mode        = "FAULT"
    state.sampling_ms = 100
    state.fan_pwm     = 100
    state.led         = "MERAH_BLINK"
    state.buzzer      = "ON"

# ═════════════════════════════════════════════════════════════════════════════
# SKENARIO ADC  — Simulasi multi-fase bermakna
# ═════════════════════════════════════════════════════════════════════════════
_stuck_value = 0

def generate_adc(tick: int, state: SystemState) -> int:
    global _stuck_value
    noise = random.randint(-15, 15)

    # Fase 1 ( 1– 25): Normal ramp-up dari ~200 → ~1500
    if tick <= 25:
        base = 200 + tick * 52

    # Fase 2 (26– 50): Masuk WARNING zone
    elif tick <= 50:
        base = 1500 + (tick - 25) * 42   # ~1500 → ~2550

    # Fase 3 (51– 70): Masuk DANGER zone
    elif tick <= 70:
        base = 2550 + (tick - 50) * 35   # ~2550 → ~3250

    # Fase 4 (71– 90): Perlahan turun
    elif tick <= 90:
        base = 3250 - (tick - 70) * 70   # ~3250 → ~1850

    # Fase 5 (91–100): Extreme jump tunggal (F3)
    elif tick == 91:
        _stuck_value = state.prev_adc
        base = state.prev_adc + 1200       # > JUMP_LIMIT → F3

    # Fase 6 (101–115): Stuck fault (F2) — nilai tidak berubah
    elif 101 <= tick <= 114:
        if _stuck_value == 0:
            _stuck_value = 600
        base = _stuck_value               # diam 14 siklus → stuck

    # Fase 7 (115): Sensor disconnect (F1)
    elif tick == 115:
        base = 4095                       # out of range → invalid

    # Fase 8 (116–150): Pulih ke normal
    else:
        t   = tick - 115
        base = int(500 + 800 * math.sin(math.pi * t / 35))

    val = int(base) + noise
    return max(10, min(4080, val))

# ═════════════════════════════════════════════════════════════════════════════
# MAIN LOOP
# ═════════════════════════════════════════════════════════════════════════════
def main():
    state = SystemState()
    rows  = []

    print("T,ADC,RISK,MODE,INT,PWM,LED,BUZZER,ERR")   # header ke stdout

    for tick in range(1, TOTAL_TICKS + 1):
        state.tick = tick

        raw            = generate_adc(tick, state)
        state.adc_raw  = raw
        state.risk_value = (raw / 4095.0) * 100.0

        valid = validate_sensor(state, raw)
        if not valid:
            set_fault(state)
        else:
            classify_risk(state)

        row = {
            "T":      tick,
            "ADC":    state.adc_raw,
            "RISK":   round(state.risk_value, 2),
            "MODE":   state.mode,
            "INT":    state.sampling_ms,
            "PWM":    state.fan_pwm,
            "LED":    state.led,
            "BUZZER": state.buzzer,
            "ERR":    state.error,
        }
        rows.append(row)

        # ── Cetak baris CSV ke stdout (identik format UART) ─────────────────
        print(f"{tick},{raw},{row['RISK']},{state.mode},"
              f"{state.sampling_ms},{state.fan_pwm},"
              f"{state.led},{state.buzzer},{state.error}")

        state.prev_adc = raw

    # ── Tulis CSV ─────────────────────────────────────────────────────────────
    fields = ["T","ADC","RISK","MODE","INT","PWM","LED","BUZZER","ERR"]
    with open(OUTPUT_CSV, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields)
        w.writeheader()
        w.writerows(rows)

    print(f"\n✓  CSV tersimpan  →  {OUTPUT_CSV}",           file=sys.stderr)
    print(f"   Jalankan       →  gnuplot plot_ars_fac.gp", file=sys.stderr)

if __name__ == "__main__":
    main()
