// =============================================================================
//  ARS-FAC — Adaptive Risk-Based Sampling & Fail-Safe Actuator Control
//  Target  : ESP32-S3 (xtensa-esp32s3-none-elf)
//  HAL     : esp-hal 0.22  |  Flash via probe-rs
//  Simulator: Proteus 8.xx — Virtual Terminal (UART0 115200 8N1)
//
//  PIN MAP:
//    GPIO1  — ADC1_CH0   → Potensiometer / DC Voltage (MQ-2 sim)  [INPUT]
//    GPIO2  — LEDC CH0   → Fan / Motor DC PWM                     [OUTPUT]
//    GPIO3  — LED Hijau  (NORMAL)                                   [OUTPUT]
//    GPIO4  — LED Kuning (WARNING)                                  [OUTPUT]
//    GPIO5  — LED Merah  (DANGER / FAULT blink)                    [OUTPUT]
//    GPIO6  — Buzzer     (DANGER / FAULT)                          [OUTPUT]
//    GPIO43 — UART0 TX   → Virtual Terminal Proteus                [OUTPUT]
//
//  SERIAL LOG FORMAT (CSV-friendly):
//    T,ADC,RISK,MODE,INT,PWM,LED,BUZZER,ERR
// =============================================================================

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::{
    analog::adc::{Adc, AdcConfig, Attenuation},
    clock::ClockControl,
    delay::Delay,
    gpio::{Io, Level, Output},
    ledc::{
        channel::{self, ChannelIFace},
        timer::{self, TimerIFace},
        LSGlobalClkSource, Ledc, LowSpeed,
    },
    peripherals::Peripherals,
    prelude::*,
};
use esp_println::println;

// =============================================================================
// TIPE DATA
// =============================================================================

/// Mode operasi sistem sesuai tabel pada flowchart
#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Normal,   // risk  0–39  | interval 2000 ms | PWM 20%
    Warning,  // risk 40–69  | interval 1000 ms | PWM 60%
    Danger,   // risk 70–100 | interval  500 ms | PWM 100%
    Fault,    // sensor invalid  | interval 100 ms | PWM 100%
}

/// Kode error sensor / kondisi abnormal
#[derive(Debug, Clone, Copy, PartialEq)]
enum ErrorType {
    None,
    SensorInvalid,    // F1 — ADC disconnect / out-of-range
    SensorStuck,      // F2 — nilai tidak berubah > N siklus
    ExtremeJump,      // F3 — delta ADC terlalu besar 1 siklus
    DangerSustained,  // F4 — risk ≥ 70 bertahan ≥ 3 siklus
}

/// Warna / kondisi LED indikator
#[derive(Debug, Clone, Copy, PartialEq)]
enum LedState {
    Green,
    Yellow,
    Red,
    RedBlink,
    Off,
}

// =============================================================================
// SYSTEM STATE — Struct untuk menyimpan seluruh state sistem
// =============================================================================

struct SystemState {
    tick:         u32,    // counter loop utama
    adc_raw:      u16,    // pembacaan ADC 0–4095
    risk_value:   f32,    // risk (%) hasil normalisasi
    mode:         Mode,
    error:        ErrorType,
    sampling_ms:  u32,    // interval adaptif (ms)
    fan_pwm_pct:  u8,     // duty cycle fan 0–100%
    led:          LedState,
    buzzer_on:    bool,
    // ── Untuk validasi sensor ─────────────────────────────────────────────
    prev_adc:     u16,
    stuck_count:  u8,
    danger_count: u8,
}

impl SystemState {
    const fn new() -> Self {
        SystemState {
            tick:         0,
            adc_raw:      0,
            risk_value:   0.0,
            mode:         Mode::Normal,
            error:        ErrorType::None,
            sampling_ms:  2000,
            fan_pwm_pct:  20,
            led:          LedState::Green,
            buzzer_on:    false,
            prev_adc:     0,
            stuck_count:  0,
            danger_count: 0,
        }
    }
}

// =============================================================================
// SENSOR VALIDATION — range check, stuck, extreme jump
// =============================================================================

/// Validasi sensor ADC. Return false → set FAULT mode.
///
/// F1: ADC ≤ 5 atau ≥ 4090  → sensor invalid / kabel lepas
/// F2: nilai sama > 10 siklus berturut → sensor stuck
/// F3: delta ADC > 1000 dalam 1 siklus → extreme jump
fn validate_sensor(state: &mut SystemState, raw: u16) -> bool {
    // F1 — Range check
    if raw <= 5 || raw >= 4090 {
        state.error = ErrorType::SensorInvalid;
        return false;
    }

    // F2 — Stuck check
    if raw == state.prev_adc {
        state.stuck_count = state.stuck_count.saturating_add(1);
        if state.stuck_count > 10 {
            state.error = ErrorType::SensorStuck;
            return false;
        }
    } else {
        state.stuck_count = 0;
    }

    // F3 — Extreme jump check
    if state.prev_adc > 0 {
        let delta = if raw > state.prev_adc {
            raw - state.prev_adc
        } else {
            state.prev_adc - raw
        };
        if delta > 1000 {
            state.error = ErrorType::ExtremeJump;
            return false;
        }
    }

    state.error = ErrorType::None;
    true
}

// =============================================================================
// RISK CLASSIFICATION — threshold-based mode assignment
// =============================================================================

/// Klasifikasi risk_value terhadap threshold dan perbarui state aktuator.
fn classify_risk(state: &mut SystemState) {
    let risk = state.risk_value;

    if risk >= 70.0 {
        // ── DANGER ────────────────────────────────────────────────────────
        state.danger_count = state.danger_count.saturating_add(1);
        state.mode        = Mode::Danger;
        state.sampling_ms = 500;
        state.fan_pwm_pct = 100;
        state.led         = LedState::Red;
        state.buzzer_on   = true;

        // F4: Danger sustained ≥ 3 siklus → flag tambahan
        if state.danger_count >= 3 {
            state.error = ErrorType::DangerSustained;
        }
    } else if risk >= 40.0 {
        // ── WARNING ───────────────────────────────────────────────────────
        state.danger_count = 0;
        state.mode         = Mode::Warning;
        state.sampling_ms  = 1000;
        state.fan_pwm_pct  = 60;
        state.led          = LedState::Yellow;
        state.buzzer_on    = false;
        state.error        = ErrorType::None;
    } else {
        // ── NORMAL ────────────────────────────────────────────────────────
        state.danger_count = 0;
        state.mode         = Mode::Normal;
        state.sampling_ms  = 2000;
        state.fan_pwm_pct  = 20;
        state.led          = LedState::Green;
        state.buzzer_on    = false;
        state.error        = ErrorType::None;
    }
}

// =============================================================================
// FAULT MODE — dipanggil saat sensor tidak valid
// =============================================================================

fn set_fault_mode(state: &mut SystemState) {
    state.mode        = Mode::Fault;
    state.sampling_ms = 100;   // immediate response
    state.fan_pwm_pct = 100;
    state.led         = LedState::RedBlink;
    state.buzzer_on   = true;
}

// =============================================================================
// HELPER — format string tanpa heap (no_std friendly)
// =============================================================================

fn mode_str(m: Mode) -> &'static str {
    match m {
        Mode::Normal  => "NORMAL",
        Mode::Warning => "WARNING",
        Mode::Danger  => "DANGER",
        Mode::Fault   => "FAULT",
    }
}

fn led_str(l: LedState) -> &'static str {
    match l {
        LedState::Green    => "HIJAU",
        LedState::Yellow   => "KUNING",
        LedState::Red      => "MERAH",
        LedState::RedBlink => "MERAH_BLINK",
        LedState::Off      => "OFF",
    }
}

fn err_str(e: ErrorType) -> &'static str {
    match e {
        ErrorType::None            => "OK",
        ErrorType::SensorInvalid   => "ERR_SENSOR_INVALID",
        ErrorType::SensorStuck     => "ERR_SENSOR_STUCK",
        ErrorType::ExtremeJump     => "ERR_EXTREME_JUMP",
        ErrorType::DangerSustained => "ERR_DANGER_SUSTAINED",
    }
}

// =============================================================================
// MAIN — Entry Point
// =============================================================================

#[entry]
fn main() -> ! {
    // ── Peripherals & Clock ──────────────────────────────────────────────────
    let peripherals = Peripherals::take();
    let system      = peripherals.SYSTEM.split();
    let clocks      = ClockControl::max(system.clock_control).freeze();
    let delay       = Delay::new(&clocks);
    let io          = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    // ── GPIO Output: LED & Buzzer ────────────────────────────────────────────
    let mut led_green  = Output::new(io.pins.gpio3, Level::Low);
    let mut led_yellow = Output::new(io.pins.gpio4, Level::Low);
    let mut led_red    = Output::new(io.pins.gpio5, Level::Low);
    let mut buzzer     = Output::new(io.pins.gpio6, Level::Low);

    // ── LEDC — Fan PWM (GPIO2, 25 kHz, 10-bit resolution) ───────────────────
    let mut ledc = Ledc::new(peripherals.LEDC, &clocks);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    let mut lstimer0 = ledc.get_timer::<LowSpeed>(timer::Number::Timer0);
    lstimer0
        .configure(timer::config::Config {
            duty:         timer::config::Duty::Duty10Bit,  // 0–1023
            clock_source: timer::LSClockSource::APBClk,
            frequency:    25u32.kHz(),
        })
        .unwrap();

    let mut fan_channel = ledc.get_channel(
        channel::Number::Channel0,
        io.pins.gpio2,
    );
    fan_channel
        .configure(channel::config::Config {
            timer:      &lstimer0,
            duty_pct:   20,   // mulai NORMAL = 20%
            pin_config: channel::config::PinConfig::PushPull,
        })
        .unwrap();

    // ── ADC1 — GPIO1 / ADC1_CH0 (Potensiometer) ─────────────────────────────
    // Attenuation 11 dB → range input 0–3.3 V → ADC 0–4095
    let mut adc1_config = AdcConfig::new();
    let mut adc1_pin    = adc1_config.enable_pin(
        io.pins.gpio1,
        Attenuation::Attenuation11dB,
    );
    let mut adc1 = Adc::new(peripherals.ADC1, adc1_config);

    // ── System State Init ────────────────────────────────────────────────────
    let mut state = SystemState::new();

    // ── CSV Header ke Virtual Terminal (resume setelah FAULT) ────────────────
    println!("=== ARS-FAC BOOT | ESP32-S3 + Rust ===");
    println!("T,ADC,RISK,MODE,INT,PWM,LED,BUZZER,ERR");

    // =========================================================================
    // MAIN LOOP
    // =========================================================================
    loop {
        state.tick = state.tick.wrapping_add(1);

        // ── STEP 1: Baca ADC → adc_raw ───────────────────────────────────────
        // Gunakan nb::block! untuk oneshot read (blocking wait)
        let raw: u16 = nb::block!(adc1.read_oneshot(&mut adc1_pin))
            .unwrap_or(0);
        state.adc_raw = raw;

        // ── STEP 2: Normalisasi → risk_value ─────────────────────────────────
        //   risk = (adc_raw / 4095.0) × 100.0
        state.risk_value = (raw as f32 / 4095.0) * 100.0;

        // ── STEP 3: Validasi Sensor ───────────────────────────────────────────
        let sensor_ok = validate_sensor(&mut state, raw);

        // ── STEP 4: Klasifikasi Risiko atau Fault ─────────────────────────────
        if sensor_ok {
            classify_risk(&mut state);
        } else {
            set_fault_mode(&mut state);
        }

        // ── STEP 5: Atur Aktuator ─────────────────────────────────────────────

        // 5a) Fan PWM — set_duty() menerima persentase 0–100
        fan_channel.set_duty(state.fan_pwm_pct).ok();

        // 5b) LED — blink menggunakan paritas tick
        let blink_on = (state.tick % 2) == 0;

        led_green.set_level(match state.led {
            LedState::Green => Level::High,
            _               => Level::Low,
        });
        led_yellow.set_level(match state.led {
            LedState::Yellow => Level::High,
            _                => Level::Low,
        });
        led_red.set_level(match state.led {
            LedState::Red                         => Level::High,
            LedState::RedBlink if blink_on        => Level::High,
            _                                     => Level::Low,
        });

        // 5c) Buzzer
        buzzer.set_level(if state.buzzer_on { Level::High } else { Level::Low });

        // ── STEP 6: Serial Log → Virtual Terminal ─────────────────────────────
        // Format: T,ADC,RISK,MODE,INT,PWM,LED,BUZZER,ERR
        //
        // Catatan: esp-println mendukung floating-point terbatas.
        // Kita pakai format integer + fraksi manual untuk keandalan.
        let risk_i = state.risk_value as u32;
        let risk_f = ((state.risk_value - risk_i as f32) * 100.0) as u32;

        println!(
            "{},{},{}.{:02},{},{},{},{},{},{}",
            state.tick,
            state.adc_raw,
            risk_i, risk_f,
            mode_str(state.mode),
            state.sampling_ms,
            state.fan_pwm_pct,
            led_str(state.led),
            if state.buzzer_on { "ON" } else { "OFF" },
            err_str(state.error)
        );

        // ── Update prev_adc setelah semua validasi selesai ───────────────────
        state.prev_adc = raw;

        // ── STEP 7: Tunggu Interval Adaptif ──────────────────────────────────
        //   delay_ms sesuai mode aktif (2000 / 1000 / 500 / 100 ms)
        delay.delay_millis(state.sampling_ms);

        // ── FAULT resume → kembali loop setelah interval 100 ms ─────────────
        // (loop otomatis kembali ke STEP 1 pada iterasi berikutnya)
    }
}
