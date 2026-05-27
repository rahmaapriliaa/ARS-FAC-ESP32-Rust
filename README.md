# ARS-FAC — Adaptive Risk-Based Sampling & Fail-Safe Actuator Control
**ESP32-S3 + Rust (esp-hal 0.22)**

---

## Sistem Singkat
- Sensor analog (potensiometer) → ADC → risk_value 0–100%
- Validasi sensor → mode NORMAL / WARNING / DANGER / FAULT
- Aktuator: Fan (PWM), LED (Hijau/Kuning/Merah), Buzzer
- Interval sampling adaptif sesuai risiko
- Serial log ke **WOKWI Virtual Terminal**

---

## PIN MAP

| GPIO | Fungsi | Komponen |
|------|--------|----------|
| 1    | ADC1_CH0 IN | Potensiometer |
| 2    | LEDC PWM OUT | Fan |
| 3    | Digital OUT | LED Hijau (NORMAL) |
| 4    | Digital OUT | LED Kuning (WARNING) |
| 5    | Digital OUT | LED Merah (DANGER/FAULT) |
| 6    | Digital OUT | Buzzer |
| 43   | UART0 TX | WOKWI Virtual Terminal |

---

## Setup Toolchain
```bash
cargo install espup
espup install
. ~/export-esp.sh

cargo install probe-rs-tools --locked
rustup target list | grep xtensa

# Development + flash via probe-rs
cargo run

# Release build
cargo build --release

cd simulation/
python simulate_ars_fac.py   # Hasilkan simulation_data.csv
gnuplot plot_ars_fac.gp       # Generate plot PNG

ars-fac-esp32s3/
├── Cargo.toml
├── build.rs
├── .cargo/config.toml
├── src/main.rs
├── simulation/
│   ├── simulate_ars_fac.py
│   └── plot_ars_fac.gp
└── README.md

T,ADC,RISK,MODE,INT,PWM,LED,BUZZER,ERR
1,820,20.02,NORMAL,2000,20,HIJAU,OFF,OK
26,1650,40.29,WARNING,1000,60,KUNING,OFF,OK
51,2870,70.08,DANGER,500,100,MERAH,ON,OK
