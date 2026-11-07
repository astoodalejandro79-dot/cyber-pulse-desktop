use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, Ordering};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};

const BASE_RADIUS: f32 = 65.0;
const MAX_EXPANSION: f32 = 110.0; // cuánto crece el orbe con bajos fuertes
const PARTICLES: usize = 16;

// --- AJUSTES (juega con estos valores) ---
const BASS_HZ: f32 = 150.0; // frecuencia de corte: solo reaccionan los graves por debajo de esto
const PUNCH: f32 = 140.0; // fuerza del "golpe" en cada bombo (más alto = más rebote)

// ---------- AUDIO ----------

/// Aísla los graves con dos filtros pasa-bajos en cascada y guarda su nivel (RMS).
struct Analyzer {
    a: f32,
    lp1: f32,
    lp2: f32,
    level: Arc<AtomicU32>,
}

impl Analyzer {
    fn new(rate: f32, level: Arc<AtomicU32>) -> Self {
        let a = 1.0 - (-TAU * BASS_HZ / rate).exp();
        Self { a, lp1: 0.0, lp2: 0.0, level }
    }

    fn feed(&mut self, samples: impl Iterator<Item = f32>) {
        let (mut sum, mut n) = (0.0f32, 0usize);
        for s in samples {
            self.lp1 += self.a * (s - self.lp1);
            self.lp2 += self.a * (self.lp1 - self.lp2);
            sum += self.lp2 * self.lp2;
            n += 1;
        }
        if n > 0 {
            let rms = (sum / n as f32).sqrt();
            if rms.is_finite() {
                self.level.store(rms.to_bits(), Ordering::Relaxed);
            }
        }
    }
}

/// Captura el audio del SISTEMA (lo que suena por los parlantes) sin configurar nada.
/// Usa `parec` con @DEFAULT_MONITOR@, que siempre apunta al monitor de la salida activa
/// (funciona con PipeWire y PulseAudio). Devuelve false si `parec` no está disponible.
fn start_monitor(level: Arc<AtomicU32>) -> bool {
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
        let mut an = Analyzer::new(44100.0, level);
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

fn start_audio(level: Arc<AtomicU32>) -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let device = host.default_input_device()?;
    let config = device.default_input_config().ok()?;
    let rate = config.sample_rate().0 as f32;
    let ch = (config.channels() as usize).max(1);
    let err_fn = |e| eprintln!("[-] Error de audio: {e}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut an = Analyzer::new(rate, level.clone());
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
            let mut an = Analyzer::new(rate, level.clone());
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

// ---------- VISUALIZADOR ----------

struct Particle {
    angle: f32,
    speed: f32,
    size: f32,
    alpha: u8,
}

struct Pulse {
    level: Arc<AtomicU32>,
    _stream: Option<cpal::Stream>,
    radius: f32,
    peak: f32,
    env: f32,
    idle: f32,
    ring1: f32,
    ring2: f32,
    ripple_r: f32,
    ripple_a: f32,
    particles: Vec<Particle>,
}

fn col(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

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

impl Pulse {
    fn new(level: Arc<AtomicU32>, stream: Option<cpal::Stream>) -> Self {
        let particles = (0..PARTICLES)
            .map(|i| Particle {
                angle: i as f32 * TAU / PARTICLES as f32,
                speed: 0.03 + 0.04 * (((i * 7) % 16) as f32 / 15.0),
                size: 1.8 + 2.4 * (((i * 5) % 16) as f32 / 15.0),
                alpha: (130 + ((i * 11) % 16) * 7) as u8,
            })
            .collect();
        Self {
            level,
            _stream: stream,
            radius: BASE_RADIUS,
            peak: 0.015,
            env: 0.0,
            idle: 0.0,
            ring1: 0.0,
            ring2: 0.0,
            ripple_r: BASE_RADIUS,
            ripple_a: 0.0,
            particles,
        }
    }

    fn step(&mut self) {
        let bass = f32::from_bits(self.level.load(Ordering::Relaxed));

        // AGC sobre los graves: el pico decae lento para adaptarse al volumen
        self.peak = (self.peak * 0.995).max(bass).max(0.004);
        let norm = (bass / self.peak).min(1.0);
        // curva que resalta los golpes fuertes y calla lo suave
        let shaped = norm.powf(1.6);

        // envolvente: sube rapidísimo (ataque) y baja con elasticidad (release)
        let prev = self.env;
        let coef = if shaped > self.env { 0.75 } else { 0.14 };
        self.env += (shaped - self.env) * coef;
        let delta = shaped - prev; // salto bruto = golpe de bombo

        let kick = delta.max(0.0) * PUNCH;
        self.idle += 0.18;
        let breath = self.idle.sin() * 2.5;

        let target = (BASE_RADIUS + self.env * MAX_EXPANSION + kick + breath)
            .clamp(BASE_RADIUS, BASE_RADIUS + MAX_EXPANSION + 30.0);

        if delta > 0.22 && self.ripple_a <= 0.0 {
            self.ripple_r = self.radius * 1.12;
            self.ripple_a = 250.0;
        }
        if self.ripple_a > 0.0 {
            self.ripple_r += 7.5;
            self.ripple_a = (self.ripple_a - 18.0).max(0.0);
        }

        // sube de golpe, vuelve con rebote suave
        let k = if target > self.radius { 0.8 } else { 0.22 };
        self.radius += (target - self.radius) * k;

        self.ring1 += 0.025;
        self.ring2 -= 0.038;
        for q in &mut self.particles {
            q.angle += q.speed;
        }
    }

    fn paint(&self, p: &egui::Painter, c: Pos2) {
        let r = self.radius;

        // ripple discontinuo
        if self.ripple_a > 0.0 {
            for i in 0..24 {
                let a0 = i as f32 * TAU / 24.0;
                arc(
                    p,
                    c,
                    self.ripple_r,
                    a0,
                    a0 + TAU / 48.0,
                    Stroke::new(2.2_f32, col(0, 255, 220, self.ripple_a as u8)),
                );
            }
        }

        // anillo HUD 1 (punteado)
        let r1 = r + 24.0;
        for i in 0..36 {
            let a = self.ring1 + i as f32 * TAU / 36.0;
            p.circle_filled(pt(c, r1, a), 1.2, col(0, 255, 220, 150));
        }

        // anillo HUD 2 (4 arcos)
        let r2 = r + 38.0;
        for i in 0..4 {
            let a0 = self.ring2 + i as f32 * TAU / 4.0;
            arc(p, c, r2, a0, a0 + TAU / 8.0, Stroke::new(1.5_f32, col(255, 0, 128, 170)));
        }

        // aro principal
        let alpha = (150.0 + r * 0.4).min(250.0) as u8;
        let w = (2.5 + (r - BASE_RADIUS) * 0.05).max(2.0);
        p.circle_stroke(c, r + 12.0, Stroke::new(w, col(255, 105, 180, alpha)));

        // cuerpo con degradado simulado (brillo arriba a la izquierda)
        p.circle_filled(c, r, col(40, 0, 50, 190));
        let layers: usize = 10;
        for i in 0..layers {
            let t = i as f32 / (layers - 1) as f32;
            let rr = r * (1.0 - 0.8 * t);
            let off = -r * 0.3 * t;
            let color = col(
                (140.0 + 115.0 * t) as u8,
                (220.0 * t * t) as u8,
                (90.0 + 150.0 * t) as u8,
                40,
            );
            p.circle_filled(c + egui::vec2(off, off), rr, color);
        }

        // partículas
        for q in &self.particles {
            p.circle_filled(pt(c, r + 18.0, q.angle), q.size, col(0, 255, 220, q.alpha));
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
    let level = Arc::new(AtomicU32::new(0));
    // 1) audio del sistema (automático); 2) si no hay `parec`, entrada por defecto
    let stream = if start_monitor(level.clone()) {
        println!("[+] Cyber Pulse: capturando el audio del sistema.");
        None
    } else {
        let s = start_audio(level.clone());
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
            .with_inner_size([550.0, 550.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_window_level(egui::WindowLevel::AlwaysOnBottom),
        ..Default::default()
    };

    eframe::run_native(
        "Cyber Pulse",
        options,
        Box::new(move |_cc| Ok(Box::new(Pulse::new(level, stream)))),
    )
}
