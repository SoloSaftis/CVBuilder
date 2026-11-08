//! Live preview: the real pages, laid out and drawn by WordCraft on a worker thread. This is the
//! same layout the Word and PDF files are written from, so what you see is what gets exported.
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use cv_core::{Project, document};

pub struct Rendered {
    pub generation: u64,
    pub pages: Vec<(usize, usize, Vec<u8>)>,
}

pub struct Preview {
    jobs: Sender<(u64, Project)>,
    results: Receiver<Rendered>,
    generation: u64,
    shown: u64,
    changed_at: Option<Instant>,
    pub textures: Vec<egui::TextureHandle>,
    pub page_count: usize,
    pub busy: bool,
}

/// Pixels per point: sharp on high-DPI screens without slowing typing down.
const SCALE: f32 = 1.6;

impl Preview {
    pub fn new(ctx: egui::Context) -> Preview {
        let (jobs, inbox) = channel::<(u64, Project)>();
        let (outbox, results) = channel();
        std::thread::Builder::new()
            .name("cv-preview".into())
            .spawn(move || {
                while let Ok(mut job) = inbox.recv() {
                    // Only the newest request matters.
                    while let Ok(newer) = inbox.try_recv() {
                        job = newer;
                    }
                    let (generation, project) = job;
                    let doc = document::build(&project);
                    let layout = wordcraft_layout::layout(&doc, &mut wordcraft_layout::LayoutCache::new(), &Default::default());
                    let options = wordcraft_render::RenderOptions::default();
                    let pages = layout
                        .pages
                        .iter()
                        .map(|page| {
                            let r = wordcraft_render::render_page(&doc, page, SCALE, &options);
                            (r.width as usize, r.height as usize, r.pixels)
                        })
                        .collect();
                    if outbox.send(Rendered { generation, pages }).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .ok();
        Preview { jobs, results, generation: 0, shown: 0, changed_at: Some(Instant::now()), textures: Vec::new(), page_count: 0, busy: false }
    }

    /// Call after any edit; the preview refreshes once typing pauses.
    pub fn changed(&mut self) {
        self.changed_at = Some(Instant::now());
    }

    pub fn tick(&mut self, ctx: &egui::Context, project: &Project) {
        if let Some(at) = self.changed_at {
            let wait = Duration::from_millis(250);
            if at.elapsed() >= wait {
                self.changed_at = None;
                self.generation += 1;
                self.busy = true;
                let _ = self.jobs.send((self.generation, project.clone()));
            } else {
                ctx.request_repaint_after(wait - at.elapsed());
            }
        }
        while let Ok(result) = self.results.try_recv() {
            if result.generation < self.shown {
                continue;
            }
            self.shown = result.generation;
            self.busy = result.generation != self.generation;
            self.page_count = result.pages.len();
            self.textures = result
                .pages
                .into_iter()
                .enumerate()
                .map(|(i, (w, h, pixels))| ctx.load_texture(format!("cv-page-{i}"), egui::ColorImage::from_rgba_premultiplied([w, h], &pixels), egui::TextureOptions::LINEAR))
                .collect();
        }
    }
}
