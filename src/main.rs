use std::f32::consts::{PI, TAU};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui::epaint::Mesh;
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};

// ---------- AJUSTES (juega con estos valores) ----------
const BASE_RADIUS: f32 = 60.0; // tamaño del orbe en reposo
const MAX_EXPANSION: f32 = 100.0; // cuánto crece con bajos fuertes
const PUNCH: f32 = 150.0; // fuerza del golpe en cada bombo (más alto = más rebote)
const PARTICLES: usize = 28;
const BARS: usize = 64; // barras del espectro circular
const WINDOW: f32 = 700.0; // tamaño de la ventana

// ---------- COLORES ----------
type Rgb = [f32; 3];
const PINK: Rgb = [255.0, 70.0, 160.0];
const CYAN: Rgb = [0.0, 255.0, 220.0];
const VIOLET: Rgb = [160.0, 80.0, 255.0];
const WHITE: Rgb = [255.0, 235.0, 250.0];

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn rgba(c: Rgb, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0] as u8, c[1] as u8, c[2] as u8, a.clamp(0.0, 255.0) as u8)
}

// ---------- ANÁLISIS DE AUDIO (8 bandas) ----------
// Bandas: <50, 50-100, 100-200 (graves), 200-400, 400-800, 800-1600, 1600-3200, >3200 Hz
const BANDS: usize = 8;
const CUTOFFS: [f32; BANDS - 1] = [50.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0];

struct Bands {
    v: [AtomicU32; BANDS],
}

impl Bands {
    fn new() -> Self {
        Self {
            v: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }

    fn get(&self, i: usize) -> f32 {
        f32::from_bits(self.v[i].load(Ordering::Relaxed))
    }
}

struct Analyzer {
    a: [f32; BANDS - 1],
    lp: [f32; BANDS - 1],
    bands: Arc<Bands>,
}

impl Analyzer {
    fn new(rate: f32, bands: Arc<Bands>) -> Self {
        let mut a = [0.0f32; BANDS - 1];
        for (i, fc) in CUTOFFS.iter().enumerate() {
            a[i] = 1.0 - (-TAU * *fc / rate).exp();
        }
        Self {
            a,
            lp: [0.0f32; BANDS - 1],
            bands,
        }
    }

    fn feed(&mut self, samples: impl Iterator<Item = f32>) {
        let mut sum = [0.0f32; BANDS];
        let mut n = 0usize;
        for s in samples {
            for i in 0..BANDS - 1 {
                self.lp[i] += self.a[i] * (s - self.lp[i]);
            }
            sum[0] += self.lp[0] * self.lp[0];
            for i in 1..BANDS - 1 {
                let d = self.lp[i] - self.lp[i - 1];
                sum[i] += d * d;
            }
            let d = s - self.lp[BANDS - 2];
            sum[BANDS - 1] += d * d;
            n += 1;
        }
        if n > 0 {
            for i in 0..BANDS {
                let rms = (sum[i] / n as f32).sqrt();
                if rms.is_finite() {
                    self.bands.v[i].store(rms.to_bits(), Ordering::Relaxed);
                }
            }
        }
    }
}

/// Captura el audio del SISTEMA (lo que suena por los parlantes) sin configurar nada.
/// Usa `parec` con @DEFAULT_MONITOR@ (PipeWire / PulseAudio).
/// Devuelve false si `parec` no está disponible.
fn start_monitor(bands: Arc<Bands>) -> bool {
    let child = Command::new("parec")
        .args([
            "-d",
            "@DEFAULT_MONITOR@",
            "--format=s16le",
            "--rate=44100",
            "--channels=1",
            "--latency-msec=20",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(_) => return false,
    };
    let mut out = match child.stdout.take() {
        Some(o) => o,
        None => return false,
    };

    std::thread::spawn(move || {
        let _child = child; // mantiene vivo el proceso mientras se lee
        let mut an = Analyzer::new(44100.0, bands);
        let mut buf = [0u8; 2048];
        loop {
            match out.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let samples = buf[..n]
                        .chunks_exact(2)
                        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0);
                    an.feed(samples);
                }
            }
        }
    });
    true
}

/// Alternativa: entrada de audio por defecto (micrófono) con cpal.
fn start_audio(bands: Arc<Bands>) -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let device = host.default_input_device()?;
    let config = device.default_input_config().ok()?;
    let rate = config.sample_rate().0 as f32;
    let ch = (config.channels() as usize).max(1);
    let err_fn = |e| eprintln!("[-] Error de audio: {e}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut an = Analyzer::new(rate, bands.clone());
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    an.feed(data.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32))
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let mut an = Analyzer::new(rate, bands.clone());
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    an.feed(data.chunks(ch).map(|f| {
                        f.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / ch as f32
                    }))
                },
                err_fn,
                None,
            )
        }
        _ => return None,
    }
    .ok()?;

    stream.play().ok()?;
    Some(stream)
}

// ---------- DIBUJO ----------

fn pt(c: Pos2, r: f32, a: f32) -> Pos2 {
    c + egui::vec2(a.cos() * r, a.sin() * r)
}

fn arc(p: &egui::Painter, c: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    let n = 24;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| pt(c, r, a0 + (a1 - a0) * i as f32 / n as f32))
        .collect();
    p.add(Shape::line(pts, stroke));
}

/// Disco con degradado radial real: color `c_in` en `focus`, `c_out` en el borde.
fn disc(p: &egui::Painter, c: Pos2, r: f32, focus: Pos2, c_in: Color32, c_out: Color32) {
    let n = 48u32;
    let mut m = Mesh::default();
    m.colored_vertex(focus, c_in);
    for i in 0..n {
        let a = i as f32 * TAU / n as f32;
        m.colored_vertex(pt(c, r, a), c_out);
    }
    for i in 0..n {
        m.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(m));
}

/// Anillo con degradado: `c0` en el radio interior, `c1` en el exterior (resplandor).
fn halo(p: &egui::Painter, c: Pos2, r0: f32, r1: f32, c0: Color32, c1: Color32) {
    let n = 64u32;
    let mut m = Mesh::default();
    for i in 0..n {
        let a = i as f32 * TAU / n as f32;
        m.colored_vertex(pt(c, r0, a), c0);
        m.colored_vertex(pt(c, r1, a), c1);
    }
    for i in 0..n {
        let a = 2 * i;
        let b = 2 * ((i + 1) % n);
        m.add_triangle(a, a + 1, b);
        m.add_triangle(b, a + 1, b + 1);
    }
    p.add(Shape::mesh(m));
}

struct Particle {
    angle: f32,
    speed: f32,
    size: f32,
    alpha: u8,
    orbit: f32,
}

struct Pulse {
    bands: Arc<Bands>,
    _stream: Option<cpal::Stream>,
    t: f32,
    radius: f32,
    peak: f32,
    env: f32,
    flash: f32,
    idle: f32,
    ring1: f32,
    ring2: f32,
    ripple_r: f32,
    ripple_a: f32,
    peaks: [f32; BANDS],
    levels: [f32; BANDS],
    bars: [f32; BARS],
    particles: Vec<Particle>,
}

impl Pulse {
    fn new(bands: Arc<Bands>, stream: Option<cpal::Stream>) -> Self {
        let particles = (0..PARTICLES)
            .map(|i| Particle {
                angle: i as f32 * TAU / PARTICLES as f32,
                speed: (0.012 + 0.03 * (((i * 7) % 16) as f32 / 15.0))
                    * (if i % 2 == 0 { 1.0 } else { -1.0 }),
                size: 1.4 + 2.2 * (((i * 5) % 16) as f32 / 15.0),
                alpha: (130 + ((i * 11) % 16) * 7) as u8,
                orbit: ((i * 3) % 8) as f32 / 7.0,
            })
            .collect();
        Self {
            bands,
            _stream: stream,
            t: 0.0,
            radius: BASE_RADIUS,
            peak: 0.015,
            env: 0.0,
            flash: 0.0,
            idle: 0.0,
            ring1: 0.0,
            ring2: 0.0,
            ripple_r: BASE_RADIUS,
            ripple_a: 0.0,
            peaks: [0.02; BANDS],
            levels: [0.0; BANDS],
            bars: [0.0; BARS],
            particles,
        }
    }

    fn step(&mut self) {
        self.t += 0.016;

        // cada banda se normaliza con su propio AGC, así todas se mueven
        for i in 0..BANDS {
            let v = self.bands.get(i);
            self.peaks[i] = (self.peaks[i] * 0.996).max(v).max(0.0015);
            let n = (v / self.peaks[i]).min(1.0).powf(1.4);
            let coef = if n > self.levels[i] { 0.7 } else { 0.12 };
            self.levels[i] += (n - self.levels[i]) * coef;
        }

        // graves = bandas 0..2 (< 200 Hz): controlan el tamaño y el golpe
        let (b0, b1, b2) = (self.bands.get(0), self.bands.get(1), self.bands.get(2));
        let bass = (b0 * b0 + b1 * b1 + b2 * b2).sqrt();
        self.peak = (self.peak * 0.995).max(bass).max(0.004);
        let norm = (bass / self.peak).min(1.0);
        let shaped = norm.powf(1.6);

        // envolvente: sube rapidísimo y baja con elasticidad
        let prev = self.env;
        let coef = if shaped > self.env { 0.75 } else { 0.14 };
        self.env += (shaped - self.env) * coef;
        let delta = shaped - prev; // salto bruto = golpe de bombo

        let kick = delta.max(0.0) * PUNCH;
        self.idle += 0.18;
        let breath = self.idle.sin() * 2.5;

        let target = (BASE_RADIUS + self.env * MAX_EXPANSION + kick + breath)
            .clamp(BASE_RADIUS, BASE_RADIUS + MAX_EXPANSION + 25.0);

        if delta > 0.22 {
            self.flash = 1.0;
            if self.ripple_a <= 0.0 {
                self.ripple_r = self.radius * 1.12;
                self.ripple_a = 250.0;
            }
        }
        self.flash *= 0.88;
        if self.ripple_a > 0.0 {
            self.ripple_r += 7.5;
            self.ripple_a = (self.ripple_a - 14.0).max(0.0);
        }

        // sube de golpe, vuelve con rebote suave
        let k = if target > self.radius { 0.8 } else { 0.22 };
        self.radius += (target - self.radius) * k;

        // barras del espectro: forma por banda + empuje general de los graves
        for i in 0..BARS {
            let mut u = 2.0 * i as f32 / BARS as f32;
            if u > 1.0 {
                u = 2.0 - u;
            }
            let x = u * (BANDS - 1) as f32;
            let i0 = x.floor() as usize;
            let i1 = (i0 + 1).min(BANDS - 1);
            let f = x - i0 as f32;
            let band = self.levels[i0] * (1.0 - f) + self.levels[i1] * f;
            let t = (band * 0.7 + self.env * 0.45).min(1.0);
            let kb = if t > self.bars[i] { 0.6 } else { 0.18 };
            self.bars[i] += (t - self.bars[i]) * kb;
        }

        self.ring1 += 0.025;
        self.ring2 -= 0.038;
        for q in &mut self.particles {
            q.angle += q.speed;
        }
    }

    fn paint(&self, p: &egui::Painter, c: Pos2) {
        let r = self.radius;
        let e = self.env;
        let fl = self.flash;

        // 1. resplandor exterior (crece con los graves)
        halo(
            p,
            c,
            r * 0.85,
            (r * 2.1).min(330.0),
            rgba(PINK, 22.0 + 80.0 * e + 40.0 * fl),
            rgba(PINK, 0.0),
        );
        halo(
            p,
            c,
            r * 0.9,
            (r * 1.5).min(330.0),
            rgba(CYAN, 10.0 + 50.0 * e),
            rgba(CYAN, 0.0),
        );

        // 2. onda expansiva en cada golpe
        if self.ripple_a > 0.0 {
            let k = self.ripple_a / 250.0;
            p.circle_stroke(
                c,
                self.ripple_r,
                Stroke::new(1.0 + 2.5 * k, rgba(CYAN, self.ripple_a)),
            );
            p.circle_stroke(
                c,
                self.ripple_r * 0.92,
                Stroke::new(1.0_f32, rgba(PINK, self.ripple_a * 0.5)),
            );
        }

        // 3. marcas giratorias
        let rt = r + 16.0;
        for i in 0..72 {
            let a = self.ring1 * 0.5 + i as f32 * TAU / 72.0;
            let len = if i % 6 == 0 { 7.0 } else { 3.0 };
            p.line_segment(
                [pt(c, rt, a), pt(c, rt + len, a)],
                Stroke::new(1.0_f32, rgba(CYAN, 90.0)),
            );
        }
        let r2 = r + 26.0;
        for i in 0..4 {
            let a0 = self.ring2 + i as f32 * TAU / 4.0;
            arc(p, c, r2, a0, a0 + TAU / 8.0, Stroke::new(1.5_f32, rgba(PINK, 150.0)));
        }

        // 4. espectro circular
        let r_in = r + 34.0;
        for i in 0..BARS {
            let a = PI / 2.0 + i as f32 * TAU / BARS as f32;
            let mut u = 2.0 * i as f32 / BARS as f32;
            if u > 1.0 {
                u = 2.0 - u;
            }
            let v = self.bars[i];
            let len = 3.0 + v * (30.0 + 28.0 * e);
            let col = mix(mix(PINK, VIOLET, u), CYAN, v * v);
            p.line_segment(
                [pt(c, r_in, a), pt(c, r_in + len, a)],
                Stroke::new(2.6_f32, rgba(col, 90.0 + 140.0 * v)),
            );
            if v > 0.35 {
                p.circle_filled(pt(c, r_in + len + 3.0, a), 1.5, rgba(WHITE, 200.0 * v));
            }
        }

        // 5. borde con degradado giratorio cian-rosa
        let seg = 96;
        let rim_r = r + 10.0;
        let w = 2.0 + 3.0 * e + 2.0 * fl;
        for i in 0..seg {
            let a0 = i as f32 * TAU / seg as f32;
            let a1 = a0 + TAU / seg as f32 * 1.08;
            let m = ((a0 * 2.0 + self.t * 2.0).sin() + 1.0) * 0.5;
            let col = mix(CYAN, PINK, m);
            p.line_segment(
                [pt(c, rim_r, a0), pt(c, rim_r, a1)],
                Stroke::new(w, rgba(col, 150.0 + 80.0 * e)),
            );
        }

        // 6. esfera de cristal translúcida
        let focus = c + egui::vec2(-r * 0.32, -r * 0.34);
        disc(
            p,
            c,
            r,
            focus,
            rgba(WHITE, 105.0),
            rgba(mix([120.0, 0.0, 110.0], VIOLET, 0.3), 60.0 + 40.0 * e),
        );
        halo(p, c, r * 0.78, r, rgba(PINK, 0.0), rgba(PINK, 70.0 + 70.0 * e));

        // núcleo que late con los graves
        let core = r * (0.30 + 0.28 * e + 0.06 * fl);
        disc(
            p,
            c,
            core,
            c,
            rgba(mix(WHITE, CYAN, 0.35), 170.0 + 80.0 * e),
            rgba(PINK, 0.0),
        );

        // brillo especular
        let hl = c + egui::vec2(-r * 0.36, -r * 0.4);
        disc(p, hl, r * 0.2, hl, rgba(WHITE, 150.0), rgba(WHITE, 0.0));

        // 7. partículas flotantes
        for q in &self.particles {
            let orbit = r + 22.0 + q.orbit * 40.0 + e * 26.0;
            let pos = pt(c, orbit, q.angle);
            p.circle_filled(pos, q.size * 2.4, rgba(CYAN, 28.0));
            p.circle_filled(pos, q.size, rgba(CYAN, q.alpha as f32));
        }

        // 8. anillo exterior punteado
        let ro = r + 104.0;
        for i in 0..48 {
            let a = -self.ring1 * 0.6 + i as f32 * TAU / 48.0;
            p.circle_filled(pt(c, ro, a), 1.2, rgba(CYAN, 70.0 + 80.0 * fl));
        }
    }
}

impl eframe::App for Pulse {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.step();
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let resp = ui.interact(rect, ui.id().with("drag"), egui::Sense::click_and_drag());
                if resp.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if resp.secondary_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                self.paint(ui.painter(), rect.center());
            });
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let bands = Arc::new(Bands::new());

    // 1) audio del sistema (automático); 2) si no hay `parec`, entrada por defecto
    let stream = if start_monitor(bands.clone()) {
        println!("[+] Cyber Pulse: capturando el audio del sistema.");
        None
    } else {
        let s = start_audio(bands.clone());
        if s.is_some() {
            println!("[+] Cyber Pulse: usando la entrada de audio por defecto.");
        } else {
            eprintln!("[-] No se pudo abrir el audio; el orbe quedará en reposo.");
        }
        s
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Cyber Pulse")
            .with_inner_size([WINDOW, WINDOW])
            .with_decorations(false)
            .with_transparent(true)
            .with_window_level(egui::WindowLevel::AlwaysOnBottom),
        ..Default::default()
    };

    eframe::run_native(
        "Cyber Pulse",
        options,
        Box::new(move |_cc| Ok(Box::new(Pulse::new(bands, stream)))),
    )
}
