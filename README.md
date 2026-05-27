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
