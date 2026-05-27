# =============================================================================
#  plot_ars_fac.gp
#  GNUPlot Visualization — ARS-FAC ESP32-S3 Rust
#
#  Usage  :  gnuplot plot_ars_fac.gp
#  Input  :  simulation_data.csv  (dari simulate_ars_fac.py atau UART capture)
#  Output :  ars_fac_risk_pwm.png  |  ars_fac_mode_transition.png
# =============================================================================

set datafile separator ","
set datafile columnheaders

# ── Warna tema gelap (sesuai tampilan flowchart) ─────────────────────────────
BG    = "#0d0d1a"
FG    = "#e0e0f0"
GRID  = "#2a2a3e"
CYAN  = "#00d4ff"
ORANGE= "#ff9944"
PURPLE= "#bb88ff"
RED   = "#ff4444"
GREEN = "#44ff88"
YELLOW= "#ffcc00"
WHITE = "#ffffff"

# =============================================================================
# PLOT 1 — risk_vs_pwm_vs_interval.png
# =============================================================================
set terminal pngcairo size 1400,900 enhanced font "DejaVu Sans,11" \
    background rgb BG
set output "ars_fac_risk_pwm.png"

set multiplot layout 3,1 \
    title "ARS-FAC | Adaptive Risk-Based Sampling & Fail-Safe Actuator Control\nESP32-S3 + Rust  |  Simulasi 150 siklus" \
    font "DejaVu Sans Bold,13" textcolor rgb WHITE

set border lc rgb FG
set tics  textcolor rgb FG
set key   textcolor rgb FG  box lc rgb GRID
set grid  lc rgb GRID lt 1 lw 0.5
set xlabel "Tick (siklus sampel)" textcolor rgb FG
set xrange [1:150]

# ── Band fill helpers ─────────────────────────────────────────────────────────
set style fill transparent solid 0.18 noborder

# ─ Panel 1: Risk Value ────────────────────────────────────────────────────────
set ylabel "Risk Value (%)" textcolor rgb CYAN
set yrange [0:115]
set title  "Risk Value vs Waktu" textcolor rgb CYAN font "DejaVu Sans Bold,12"

# Zona warna background
set object 1 rect from 1,70  to 150,100 fc rgb RED    fs transparent solid 0.12 noborder
set object 2 rect from 1,40  to 150,70  fc rgb YELLOW fs transparent solid 0.10 noborder
set object 3 rect from 1,0   to 150,40  fc rgb GREEN  fs transparent solid 0.08 noborder

# Label zona
set label 1 "DANGER  ≥70"  at 2, 86 tc rgb "#ff8888" font "DejaVu Sans,9"
set label 2 "WARNING 40–69" at 2, 56 tc rgb "#ffdd66" font "DejaVu Sans,9"
set label 3 "NORMAL  <40"  at 2, 22 tc rgb "#88ffaa" font "DejaVu Sans,9"

# Garis threshold
set arrow 1 from 1,70  to 150,70  nohead lc rgb RED    lt 2 lw 1.2
set arrow 2 from 1,40  to 150,40  nohead lc rgb YELLOW lt 2 lw 1.2

plot "simulation_data.csv" using 1:3 \
     with lines lw 2.5 lc rgb CYAN dt 1 title "risk\_value (%)"

unset object 1
unset object 2
unset object 3
unset label 1
unset label 2
unset label 3
unset arrow 1
unset arrow 2

# ─ Panel 2: Fan PWM ──────────────────────────────────────────────────────────
set ylabel "Fan PWM (%)"  textcolor rgb ORANGE
set yrange [0:115]
set title  "Fan PWM vs Waktu (Aktuator Respons)" \
           textcolor rgb ORANGE font "DejaVu Sans Bold,12"

# Garis referensi PWM
set arrow 3 from 1,100 to 150,100 nohead lc rgb RED    lt 2 lw 1.0
set arrow 4 from 1,60  to 150,60  nohead lc rgb YELLOW lt 2 lw 1.0
set arrow 5 from 1,20  to 150,20  nohead lc rgb GREEN  lt 2 lw 1.0

set label 4 "100% (DANGER/FAULT)"  at 2,103 tc rgb "#ff8888" font "DejaVu Sans,8"
set label 5 "60% (WARNING)"        at 2,63  tc rgb "#ffdd66" font "DejaVu Sans,8"
set label 6 "20% (NORMAL)"         at 2,23  tc rgb "#88ffaa" font "DejaVu Sans,8"

plot "simulation_data.csv" using 1:6 \
     with steps lw 2.5 lc rgb ORANGE dt 1 title "fan\_pwm (%)"

unset arrow 3
unset arrow 4
unset arrow 5
unset label 4
unset label 5
unset label 6

# ─ Panel 3: Sampling Interval ────────────────────────────────────────────────
set ylabel "Interval (ms)"  textcolor rgb PURPLE
set yrange [0:2300]
set title  "Adaptive Sampling Interval vs Waktu" \
           textcolor rgb PURPLE font "DejaVu Sans Bold,12"

set arrow 6 from 1,2000 to 150,2000 nohead lc rgb GREEN  lt 2 lw 1.0
set arrow 7 from 1,1000 to 150,1000 nohead lc rgb YELLOW lt 2 lw 1.0
set arrow 8 from 1,500  to 150,500  nohead lc rgb RED    lt 2 lw 1.0

set label 7 "2000 ms"  at 2,2050 tc rgb "#88ffaa" font "DejaVu Sans,8"
set label 8 "1000 ms"  at 2,1050 tc rgb "#ffdd66" font "DejaVu Sans,8"
set label 9 " 500 ms"  at 2, 550 tc rgb "#ff8888" font "DejaVu Sans,8"
set label 10 "100 ms"  at 2, 140 tc rgb "#ff4444" font "DejaVu Sans,8"

plot "simulation_data.csv" using 1:5 \
     with steps lw 2.5 lc rgb PURPLE dt 1 title "interval\_ms"

unset multiplot
set output
print "✓  Saved: ars_fac_risk_pwm.png"

# =============================================================================
# PLOT 2 — mode_transition.png  (mode sebagai nilai numerik)
# =============================================================================
set terminal pngcairo size 1400,480 enhanced font "DejaVu Sans,11" \
    background rgb BG
set output "ars_fac_mode_transition.png"

# Encode mode ke angka: 0=NORMAL 1=WARNING 2=DANGER 3=FAULT
# Gunakan ternary GNUPlot dengan word() dan stringcolumn
set yrange [-0.3:3.5]
set xrange [1:150]
set ytics  ("NORMAL" 0, "WARNING" 1, "DANGER" 2, "FAULT" 3) textcolor rgb FG
set title  "Mode Transition Timeline — ARS-FAC" \
           textcolor rgb WHITE font "DejaVu Sans Bold,12"
set xlabel "Tick" textcolor rgb FG
set border  lc rgb FG
set tics    textcolor rgb FG
set key off
set grid    lc rgb GRID lt 1 lw 0.5

# Map string MODE ke integer via conditional filter plot
plot \
  "simulation_data.csv" using 1:( strcol(4) eq "NORMAL"  ? 0 : 1/0) \
      with points pt 7 ps 1.2 lc rgb GREEN  title "NORMAL",  \
  "simulation_data.csv" using 1:( strcol(4) eq "WARNING" ? 1 : 1/0) \
      with points pt 7 ps 1.2 lc rgb YELLOW title "WARNING", \
  "simulation_data.csv" using 1:( strcol(4) eq "DANGER"  ? 2 : 1/0) \
      with points pt 7 ps 1.2 lc rgb RED    title "DANGER",  \
  "simulation_data.csv" using 1:( strcol(4) eq "FAULT"   ? 3 : 1/0) \
      with points pt 7 ps 1.2 lc rgb ORANGE title "FAULT"

set output
print "✓  Saved: ars_fac_mode_transition.png"
print ""
print "=== SELESAI ==="
print "File yang dihasilkan:"
print "  ars_fac_risk_pwm.png       (3-panel: risk / pwm / interval)"
print "  ars_fac_mode_transition.png (timeline mode state)"
