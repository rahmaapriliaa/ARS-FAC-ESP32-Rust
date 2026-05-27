//! ARS-FAC — Adaptive Risk-Based Sampling & Fail-Safe Actuator Control
//! ESP32-S3 + Rust | Wokwi VS Code
//!
//! PIN MAP:
//!   GPIO1  → ADC  : Potensiometer (simulasi MQ-2)
//!   GPIO4  → LED Hijau    (NORMAL)
//!   GPIO5  → LED Kuning   (WARNING)
//!   GPIO6  → LED Merah    (DANGER / FAULT blink)
//!   GPIO7  → Buzzer       (DANGER / FAULT)
//!   GPIO8  → PWM Fan      (20% / 60% / 100%)

use esp_idf_svc::hal::{
    adc::{
        attenuation::DB_11,
        oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
    },
    delay::FreeRtos,
    gpio::PinDriver,
    ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution},
    peripherals::Peripherals,
    units::FromValueType,
};

// ════════════════════════════════════════════════
//  KONSTANTA THRESHOLD
// ════════════════════════════════════════════════
const ADC_MAX:        f32 = 4095.0;
const RISK_WARN:      f32 = 40.0;   // batas Normal → Warning
const RISK_DANGER:    f32 = 70.0;   // batas Warning → Danger
const JUMP_THRESHOLD: i32 = 2500;   // F3: delta ADC max 1 siklus
const STUCK_MAX:      u32 = 10;     // F2: siklus tidak berubah
const DANGER_MAX:     u32 = 5;      // F4: siklus danger → fault

// ════════════════════════════════════════════════
//  ENUM MODE — 4 STATE
// ════════════════════════════════════════════════
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Normal,   // risk  0–39  | interval 2000ms | PWM 20%  | LED Hijau
    Warning,  // risk 40–69  | interval 1000ms | PWM 60%  | LED Kuning
    Danger,   // risk 70–100 | interval  500ms | PWM 100% | LED Merah
    Fault,    // invalid     | interval  100ms | PWM 100% | LED Merah Blink
}

// ════════════════════════════════════════════════
//  ENUM ERROR TYPE
// ════════════════════════════════════════════════
#[derive(Clone, Copy, PartialEq, Debug)]
enum ErrType {
    None,
    SensorInvalid,    // F1: disconnected / out-of-range
    SensorStuck,      // F2: nilai tidak berubah > STUCK_MAX siklus
    ExtremeJump,      // F3: naik/turun drastis dalam 1 siklus
    DangerSustained,  // F4: risk >= 70 selama DANGER_MAX siklus
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Normal  => "NORMAL",
            Mode::Warning => "WARNING",
            Mode::Danger  => "DANGER",
            Mode::Fault   => "FAULT",
        }
    }
}

impl ErrType {
    fn as_str(self) -> &'static str {
        match self {
            ErrType::None            => "OK",
            ErrType::SensorInvalid   => "ERR_SENSOR_INVALID",
            ErrType::SensorStuck     => "ERR_SENSOR_STUCK",
            ErrType::ExtremeJump     => "ERR_EXTREME_JUMP",
            ErrType::DangerSustained => "ERR_DANGER_SUSTAINED",
        }
    }
}

// ════════════════════════════════════════════════
//  STRUCT SYSTEM STATE
// ════════════════════════════════════════════════
struct SystemState {
    tick:         u32,
    adc_raw:      u16,
    risk_value:   f32,
    mode:         Mode,
    error:        ErrType,
    interval_ms:  u32,
    pwm_pct:      u32,
    led_str:      &'static str,
    buz_str:      &'static str,
    // internal counter
    prev_adc:     i32,
    stuck_cnt:    u32,
    danger_cnt:   u32,
    blink_state:  bool,
}

impl SystemState {
    fn new() -> Self {
        Self {
            tick:        0,
            adc_raw:     0,
            risk_value:  0.0,
            mode:        Mode::Normal,
            error:       ErrType::None,
            interval_ms: 2000,
            pwm_pct:     20,
            led_str:     "GREEN",
            buz_str:     "OFF",
            prev_adc:    -1,
            stuck_cnt:   0,
            danger_cnt:  0,
            blink_state: false,
        }
    }
}

// ════════════════════════════════════════════════
//  STEP 1: VALIDASI SENSOR
//  Sesuai flowchart: cek range, stuck, extreme jump
// ════════════════════════════════════════════════
fn validate_sensor(state: &mut SystemState) -> ErrType {
    let raw  = state.adc_raw;
    let prev = state.prev_adc;

    // F1 — Sensor disconnected / out-of-range
    if raw < 10 || raw > 4085 {
        state.stuck_cnt = 0;
        return ErrType::SensorInvalid;
    }

    // F2 — ADC Stuck: nilai tidak berubah terlalu lama
    if raw as i32 == prev {
        state.stuck_cnt += 1;
        if state.stuck_cnt >= STUCK_MAX {
            return ErrType::SensorStuck;
        }
    } else {
        state.stuck_cnt = 0;
    }

    // F3 — Extreme Jump: delta terlalu besar dalam 1 siklus
    if prev >= 0 && (raw as i32 - prev).abs() > JUMP_THRESHOLD {
        return ErrType::ExtremeJump;
    }

    ErrType::None
}

// ════════════════════════════════════════════════
//  STEP 2: NORMALISASI
//  risk = (adc_raw / 4095.0) × 100.0
// ════════════════════════════════════════════════
fn normalize(raw: u16) -> f32 {
    (raw as f32 / ADC_MAX) * 100.0
}

// ════════════════════════════════════════════════
//  STEP 3: KLASIFIKASI RISIKO → 4 STATE
// ════════════════════════════════════════════════
fn classify_risk(state: &mut SystemState) {
    // Jika ada error → langsung FAULT
    if state.error != ErrType::None {
        state.mode        = Mode::Fault;
        state.interval_ms = 100;
        state.pwm_pct     = 100;
        state.led_str     = "RED_BLINK";
        state.buz_str     = "ON";
        return;
    }

    let risk = state.risk_value;

    if risk < RISK_WARN {
        // ── STATE: NORMAL ──────────────────────
        state.mode        = Mode::Normal;
        state.interval_ms = 2000;
        state.pwm_pct     = 20;
        state.led_str     = "GREEN";
        state.buz_str     = "OFF";
        state.danger_cnt  = 0;

    } else if risk < RISK_DANGER {
        // ── STATE: WARNING ─────────────────────
        state.mode        = Mode::Warning;
        state.interval_ms = 1000;
        state.pwm_pct     = 60;
        state.led_str     = "YELLOW";
        state.buz_str     = "OFF";
        state.danger_cnt  = 0;

    } else {
        // ── STATE: DANGER ──────────────────────
        state.danger_cnt += 1;
        state.mode        = Mode::Danger;
        state.interval_ms = 500;
        state.pwm_pct     = 100;
        state.led_str     = "RED";
        state.buz_str     = "ON";

        // F4 — Danger Sustained → eskalasi ke FAULT
        if state.danger_cnt >= DANGER_MAX {
            state.mode    = Mode::Fault;
            state.error   = ErrType::DangerSustained;
            state.interval_ms = 100;
            state.led_str = "RED_BLINK";
        }
    }
}

// ════════════════════════════════════════════════
//  ENTRY POINT
// ════════════════════════════════════════════════
fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let p = Peripherals::take().unwrap();

    // ── GPIO LED & Buzzer ────────────────────────
    let mut led_green  = PinDriver::output(p.pins.gpio4).unwrap();
    let mut led_yellow = PinDriver::output(p.pins.gpio5).unwrap();
    let mut led_red    = PinDriver::output(p.pins.gpio6).unwrap();
    let mut buzzer     = PinDriver::output(p.pins.gpio7).unwrap();

    // ── LEDC PWM Fan GPIO8 ───────────────────────
    let timer = LedcTimerDriver::new(
        p.ledc.timer0,
        &TimerConfig::default()
            .frequency(25_000.Hz())
            .resolution(Resolution::Bits8),
    ).unwrap();
    let mut fan = LedcDriver::new(p.ledc.channel0, &timer, p.pins.gpio8).unwrap();
    let max_duty = fan.get_max_duty();

    // ── ADC GPIO1 ────────────────────────────────
    let adc  = AdcDriver::new(p.adc1).unwrap();
    let cfg  = AdcChannelConfig { attenuation: DB_11, ..Default::default() };
    let mut apin = AdcChannelDriver::new(&adc, p.pins.gpio1, &cfg).unwrap();

    // ── Header CSV ───────────────────────────────
    println!("T,ADC,RISK,MODE,INT_MS,PWM%,LED,BUZZER,ERR");

    // ── Inisialisasi state ───────────────────────
    let mut s = SystemState::new();

    // ════════════════════════════════════════════
    //  MAIN LOOP — sesuai flowchart
    // ════════════════════════════════════════════
    loop {
        // ① Baca ADC
        s.adc_raw = adc.read(&mut apin).unwrap_or(0);

        // ② Validasi sensor (F1/F2/F3)
        s.error = validate_sensor(&mut s);

        // ③ Normalisasi (hanya jika valid)
        if s.error == ErrType::None {
            s.risk_value = normalize(s.adc_raw);
        }

        // ④ Klasifikasi risiko → set mode + params
        classify_risk(&mut s);

        // ⑤ Set PWM Fan
        fan.set_duty((max_duty * s.pwm_pct) / 100).unwrap();

        // ⑥ Set LED sesuai mode
        led_green.set_level( (s.mode == Mode::Normal).into()).unwrap();
        led_yellow.set_level((s.mode == Mode::Warning).into()).unwrap();

        if s.mode == Mode::Fault {
            // LED merah blink
            s.blink_state = !s.blink_state;
            led_red.set_level(s.blink_state.into()).unwrap();
        } else {
            led_red.set_level((s.mode == Mode::Danger).into()).unwrap();
        }

        // ⑦ Set Buzzer
        let buz_on = s.mode == Mode::Danger || s.mode == Mode::Fault;
        buzzer.set_level(buz_on.into()).unwrap();

        // ⑧ Kirim log serial → Wokwi Serial Monitor → CSV
        println!("{},{},{:.1},{},{},{},{},{},{}",
            s.tick,
            s.adc_raw,
            s.risk_value,
            s.mode.as_str(),
            s.interval_ms,
            s.pwm_pct,
            s.led_str,
            s.buz_str,
            s.error.as_str(),
        );

        // Update prev_adc untuk siklus berikutnya
        s.prev_adc = s.adc_raw as i32;
        s.tick += 1;

        // ⑨ Tunggu interval adaptif sesuai mode
        FreeRtos::delay_ms(s.interval_ms);
    }
}