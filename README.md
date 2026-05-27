# ARS-FAC — Adaptive Risk-Based Sampling & Fail-Safe Actuator Control
**ESP32-S3 + Rust (esp-hal 0.22) | Kelas 4C**

---

## Gambaran Sistem

```
Potensiometer / DC Voltage (GPIO1)
        │
        ▼
  [ADC1_CH0 12-bit]  →  adc_raw (0–4095)
        │
  Normalisasi: risk = (adc_raw / 4095.0) × 100.0
        │
  Validasi Sensor ──FAULT──► Fan 100% + Buzzer + LED Blink
        │ valid
  Klasifikasi Risiko
   ├─ <40  → NORMAL   (2000 ms, PWM 20%, LED Hijau)
   ├─ 40–69 → WARNING  (1000 ms, PWM 60%, LED Kuning)
   └─ ≥70  → DANGER   ( 500 ms, PWM 100%, LED Merah, Buzzer)
        │
  Atur Aktuator (GPIO2=Fan PWM, GPIO3/4/5=LED, GPIO6=Buzzer)
        │
  Serial Log → UART0 (Virtual Terminal Proteus)
  T,ADC,RISK,MODE,INT,PWM,LED,BUZZER,ERR
        │
  delay_ms(sampling_ms)  ←─ interval adaptif
        └─────────────────────────────────────────► loop
```

---

## PIN MAP

| GPIO | Fungsi        | Komponen Proteus          |
|------|---------------|---------------------------|
| 1    | ADC1_CH0 IN   | Potensiometer / DC Source |
| 2    | LEDC PWM OUT  | DC Motor (Fan)            |
| 3    | Digital OUT   | LED Hijau (NORMAL)        |
| 4    | Digital OUT   | LED Kuning (WARNING)      |
| 5    | Digital OUT   | LED Merah (DANGER/FAULT)  |
| 6    | Digital OUT   | Buzzer                    |
| 43   | UART0 TX      | Virtual Terminal          |

---

## Setup Toolchain

### 1. Install espup (Rust for Xtensa)
```bash
cargo install espup
espup install
# Tambahkan ke shell profile:
. ~/export-esp.sh
```

### 2. Install probe-rs
```bash
cargo install probe-rs-tools --locked
```

### 3. Verifikasi target tersedia
```bash
rustup target list | grep xtensa
# Harus muncul: xtensa-esp32s3-none-elf
```

---

## Build & Flash

```bash
# Development build + flash via probe-rs (JTAG/USB)
cargo run

# Release build saja
cargo build --release

# Flash manual ke alamat 0x0
probe-rs run --chip esp32s3 target/xtensa-esp32s3-none-elf/release/ars-fac-esp32s3
```

---

## Simulasi Proteus

1. Buka Proteus 8.xx, buat schematic ESP32-S3
2. Tambahkan **Virtual Terminal** (UART0, 115200 8N1)
3. Hubungkan potensiometer ke GPIO1
4. Load `.elf` file dari `target/xtensa-esp32s3-none-elf/debug/ars-fac-esp32s3`
5. Run → output CSV muncul di Virtual Terminal

---

## Simulasi Software + GNUPlot

Tanpa hardware, jalankan simulasi Python:

```bash
cd simulation/

# Hasilkan simulation_data.csv (150 siklus, multi-fase + fault injection)
python simulate_ars_fac.py

# Generate 2 plot PNG
gnuplot plot_ars_fac.gp
```

Output file:
- `simulation_data.csv`
- `ars_fac_risk_pwm.png`         — 3-panel: risk / PWM / interval
- `ars_fac_mode_transition.png`  — timeline transisi mode

---

## Format Log Serial (CSV)

```
T,ADC,RISK,MODE,INT,PWM,LED,BUZZER,ERR
1,820,20.02,NORMAL,2000,20,HIJAU,OFF,OK
26,1650,40.29,WARNING,1000,60,KUNING,OFF,OK
51,2870,70.08,DANGER,500,100,MERAH,ON,OK
```

| Kolom  | Keterangan                            |
|--------|---------------------------------------|
| T      | Nomor tick / iterasi loop             |
| ADC    | Nilai raw ADC 0–4095                  |
| RISK   | Risk value (%) dua desimal            |
| MODE   | NORMAL / WARNING / DANGER / FAULT     |
| INT    | Sampling interval aktif (ms)          |
| PWM    | Duty cycle fan (%)                    |
| LED    | HIJAU / KUNING / MERAH / MERAH_BLINK  |
| BUZZER | ON / OFF                              |
| ERR    | OK / ERR_SENSOR_INVALID / ...         |

---

## Kode Error Sensor (F1–F4)

| Kode | ErrorType              | Kondisi Pemicu                         |
|------|------------------------|----------------------------------------|
| F1   | ERR_SENSOR_INVALID     | ADC ≤5 atau ≥4090 (kabel lepas/short) |
| F2   | ERR_SENSOR_STUCK       | Nilai sama >10 siklus berturut         |
| F3   | ERR_EXTREME_JUMP       | Delta ADC >1000 dalam 1 siklus         |
| F4   | ERR_DANGER_SUSTAINED   | risk ≥70 bertahan ≥3 siklus            |

---

## Struktur Project

```
ars-fac-esp32s3/
├── Cargo.toml
├── build.rs
├── .cargo/
│   └── config.toml          ← target + runner probe-rs
├── src/
│   └── main.rs              ← kode utama (ADC, PWM, LED, Buzzer, UART log)
├── simulation/
│   ├── simulate_ars_fac.py  ← simulasi software → CSV
│   └── plot_ars_fac.gp      ← GNUPlot script → PNG
└── README.md
```

---

## Referensi Paper Pendukung

| # | Paper | Relevansi |
|---|-------|-----------|
| 1 | Başaran (2025) — Ground Fault Detection ECU | Inspirasi fail-safe PWM shutdown |
| 2 | Qin et al. (2024) — Safety Issues in Rust | Unsafe code & panic mitigation |
| 3 | Babiuch & Smutný (2026) — LLM for ESP32 | Benchmark embedded code quality |
| 8 | Lee & Kim (2021) — ASMP Adaptive Sampling | Dasar ASA-m adaptive interval |
| 10 | Solouki et al. (2024) — Fault Tolerance Survey | Software-based fault mitigation |
| 12 | Daupayev et al. (2025) — Two-to-One Trigger | Event-based threshold sampling |
