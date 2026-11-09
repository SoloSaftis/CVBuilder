//! Click and overlap checks for every control in the window, run headless with egui_kittest.
//!
//! For each screen (ribbon tab, side panel, dialog), in each interface language asked for and at
//! two window sizes, every enabled control is scrolled into view and the pointer is put on its
//! centre and near its four corners: egui must give the click to that control and nothing else
//! (the bug where a section's colour button folded the section instead). Dropdowns and menus are
//! opened and their items checked the same way. Visible text that overlaps other text or a
//! control is listed.
//!
//! Slow, so ignored in the normal run:
//! `cargo test --release -p cvbuilder-app ui_qa -- --ignored --nocapture --test-threads=1`
//! `CVBUILDER_QA=<part of a scene name>` runs matching scenes; `CVBUILDER_QA_LANGS=no,en,de`
//! picks the interface languages. It works in a throwaway portable data folder next to the
//! test program, never in the person's own.
use std::collections::{BTreeSet, HashMap};

use egui::accesskit::{Action, Role};
use egui::{Id, Pos2, Rect, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable, by};

use crate::{CvApp, Side, i18n, menu_ui::Tab};

/// One control (or text) as accessibility sees it. `rect` is not clipped by scrolling.
#[derive(Clone, Debug)]
struct Ctl {
    id: u64,
    label: String,
    role: Role,
    rect: Rect,
    /// A control: a button, field, box … (not plain text).
    control: bool,
}

fn snapshot(h: &Harness<'_, CvApp>) -> Vec<Ctl> {
    let ppp = h.ctx.pixels_per_point();
    let mut out = Vec::new();
    for n in h.query_all(by().include_labels().predicate(|_| true)) {
        let ak = n.accesskit_node();
        let Some(b) = ak.bounding_box() else { continue };
        let rect = Rect::from_min_max(Pos2::new(b.x0 as f32 / ppp, b.y0 as f32 / ppp), Pos2::new(b.x1 as f32 / ppp, b.y1 as f32 / ppp));
        let label = ak.label().or_else(|| ak.value()).unwrap_or_default().replace('\n', " ");
        let role = ak.role();
        out.push(Ctl {
            id: ak.locate().0.0,
            label: if label.chars().count() > 60 { label.chars().take(57).collect::<String>() + "…" } else { label },
            role,
            rect,
            control: role != Role::Label && ak.data().supports_action(Action::Click) && !ak.is_disabled() && !ak.is_hidden(),
        });
    }
    out
}

fn step(h: &mut Harness<'_, CvApp>, n: usize) {
    for _ in 0..n {
        h.step();
    }
}

fn probe_points(r: Rect) -> [Pos2; 5] {
    let inset = 2.0_f32.min(r.width() / 4.0).min(r.height() / 4.0);
    let r = r.shrink(inset);
    [r.center(), r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()]
}

/// The egui id and the visible (clipped) area of a node, found by putting the pointer on it.
/// None when no part of it can be reached (scrolled away or covered).
fn resolve(h: &mut Harness<'_, CvApp>, c: &Ctl, screen: Rect) -> Option<(Id, Rect)> {
    let visible = c.rect.intersect(screen);
    if visible.width() < 2.0 || visible.height() < 2.0 {
        return None;
    }
    for p in probe_points(visible) {
        h.hover_at(p);
        step(h, 1);
        let found = h.ctx.interaction_snapshot(|i| i.contains_pointer.iter().copied().find(|id| id.value() == c.id));
        if let Some(id) = found {
            // The visible widget (egui widens the click area into the gap between neighbours),
            // cut to what scrolling leaves in view.
            let resp = h.ctx.read_response(id)?;
            let r = resp.rect.intersect(resp.interact_rect).intersect(screen);
            return (r.width() >= 2.0 && r.height() >= 2.0).then_some((id, r));
        }
    }
    None
}

/// Which control would get a click at `pos`.
fn click_target(h: &mut Harness<'_, CvApp>, pos: Pos2) -> Option<Id> {
    h.hover_at(pos);
    step(h, 1);
    let hovered: Vec<Id> = h.ctx.interaction_snapshot(|i| i.hovered.iter().copied().collect());
    // A dialog's content sits in a layer that senses clicks and drags (so nothing falls through
    // to the backdrop); it can be listed as hovered for the drag while the click goes to the
    // control inside. The innermost click-sensing widget is the one that gets the click.
    hovered.into_iter().filter_map(|id| h.ctx.read_response(id).filter(|r| r.sense.senses_click()).map(|r| (id, r.rect.area()))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id)
}

fn name(ctls: &HashMap<u64, Ctl>, id: u64) -> String {
    match ctls.get(&id) {
        Some(c) if !c.label.is_empty() => format!("«{}» ({:?})", c.label, c.role),
        Some(c) => format!("[{:?} without label at {:.0},{:.0}]", c.role, c.rect.min.x, c.rect.min.y),
        None => format!("[id {id:x}]"),
    }
}

/// Tests the click area of every control in `ids`; returns the problems.
fn check_clicks(h: &mut Harness<'_, CvApp>, ids: &[u64], screen: Rect) -> Vec<String> {
    let mut problems = Vec::new();
    for &id in ids {
        // Into view first: the form and side panels scroll.
        if let Some(n) = h.query_all(by().include_labels().predicate(move |n| n.locate().0.0 == id)).next() {
            n.scroll_to_me();
        }
        // Wait until it has stopped moving (a scroll can land over a few frames).
        let mut last = None;
        let mut same = 0;
        for _ in 0..40 {
            step(h, 1);
            let now = h.query_all(by().include_labels().predicate(move |n| n.locate().0.0 == id)).next().and_then(|n| n.accesskit_node().bounding_box()).map(|b| (b.x0, b.y0, b.x1, b.y1));
            same = if now.is_some() && now == last { same + 1 } else { 0 };
            if same >= 3 {
                break;
            }
            last = now;
        }
        let all: HashMap<u64, Ctl> = snapshot(h).into_iter().map(|c| (c.id, c)).collect();
        let Some(c) = all.get(&id) else { continue };
        let Some((egui_id, r)) = resolve(h, c, screen) else {
            problems.push(format!("{} can't be reached (covered, or not scrolled into view)", name(&all, id)));
            continue;
        };
        let mut wrong: BTreeSet<String> = BTreeSet::new();
        let mut dead = 0;
        let debug = std::env::var("CVBUILDER_QA_DEBUG").is_ok_and(|d| c.label.contains(&d));
        if debug {
            println!("DEBUG {} ak={:?} resolved={:?} pass={}", c.label, c.rect, r, h.ctx.cumulative_pass_nr());
        }
        for p in probe_points(r) {
            let t = click_target(h, p);
            if debug {
                let tr = t.and_then(|t| h.ctx.read_response(t)).map(|x| (x.rect, x.interact_rect));
                println!("   at {p:?} -> {:?} {:?} self now {:?}", t.map(|t| name(&all, t.value())), tr, h.ctx.read_response(egui_id).map(|x| x.rect));
            }
            match t {
                Some(t) if t == egui_id => {}
                Some(t) => {
                    wrong.insert(name(&all, t.value()));
                }
                None => dead += 1,
            }
        }
        if !wrong.is_empty() {
            problems.push(format!("{}: part of it clicks {}", name(&all, id), wrong.into_iter().collect::<Vec<_>>().join(", ")));
        }
        if dead > 0 {
            problems.push(format!("{}: {dead} of 5 points inside it click nothing", name(&all, id)));
        }
    }
    h.remove_cursor();
    step(h, 1);
    problems
}

/// Visible text that overlaps other text or a control. A text lying inside a control (its
/// label, a fold mark) is part of it and fine; a partial overlap is not.
fn check_overlaps(h: &mut Harness<'_, CvApp>, ctls: &[Ctl], screen: Rect) -> Vec<String> {
    let all: HashMap<u64, Ctl> = ctls.iter().map(|c| (c.id, c.clone())).collect();
    let mut seen: Vec<(u64, Rect, bool)> = Vec::new();
    // Resize edges are thin invisible strips: the click check covers them, not this one.
    for c in ctls.iter().filter(|c| c.role != Role::Splitter && (c.control || (c.role == Role::Label && !c.label.trim().is_empty()))) {
        if let Some((_, r)) = resolve(h, c, screen) {
            seen.push((c.id, r, c.control));
        }
    }
    let mut problems = BTreeSet::new();
    for (i, &(a, ra, a_control)) in seen.iter().enumerate() {
        for &(b, rb, b_control) in &seen[i + 1..] {
            if a_control && b_control {
                continue; // two controls: the click check judges those
            }
            let x = ra.intersect(rb);
            if x.width() <= 1.0 || x.height() <= 1.0 {
                continue;
            }
            let inside = |inner: Rect, outer: Rect| inner.intersect(outer).area() >= inner.area() * 0.9;
            if inside(ra, rb) || inside(rb, ra) {
                continue;
            }
            if x.area() / ra.area().min(rb.area()).max(1.0) > 0.15 {
                problems.insert(format!("{} overlaps {}", name(&all, a), name(&all, b)));
            }
        }
    }
    h.remove_cursor();
    step(h, 1);
    problems.into_iter().collect()
}

/// What opens a dropdown or a menu: tested by opening it and checking its items.
fn opens_popup(c: &Ctl) -> bool {
    const MENUS: [&str; 14] = ["Fil", "File", "Seksjon", "Section", "Marger", "Margins", "Størrelse", "Size", "Ny side", "New page", "Spalter", "Columns", "Palett", "Palette"];
    // Ribbon tools start with an icon from the icon font, and the ones with a menu end with ▾.
    let label = c.label.trim_start_matches(|ch: char| crate::theme::ph::is_icon(ch) || ch.is_whitespace());
    let caret = c.label.trim_end().ends_with(crate::theme::ph::CARET_DOWN);
    c.control && (c.role == Role::ComboBox || caret || MENUS.iter().any(|m| label.starts_with(m)))
}

struct Scene {
    name: &'static str,
    setup: fn(&mut CvApp),
}

const SCENES: &[Scene] = &[
    Scene { name: "Hjem", setup: |_| {} },
    Scene { name: "Sett inn", setup: |a| a.ribbon_tab = Tab::Insert },
    Scene { name: "Sideoppsett", setup: |a| a.ribbon_tab = Tab::Page },
    Scene { name: "Design", setup: |a| a.ribbon_tab = Tab::Design },
    Scene { name: "Vis", setup: |a| a.ribbon_tab = Tab::View },
    Scene { name: "AI-fanen", setup: |a| a.ribbon_tab = Tab::Ai },
    Scene { name: "Hjelp", setup: |a| a.ribbon_tab = Tab::Help },
    Scene { name: "Sjekkliste", setup: |a| a.side = Side::Checks },
    Scene { name: "Alle seksjoner åpne", setup: |a| a.folded.clear() },
    Scene { name: "Oversett-vinduet", setup: |a| a.open_translate(None) },
    Scene { name: "Innstillinger", setup: |a| a.settings_open = true },
    Scene { name: "Ny seksjon", setup: |a| a.new_section = Some(String::new()) },
    Scene { name: "Sjekklisteredigering", setup: |a| a.checklist.editing = Some(cv_core::checklist::Checklist::starter()) },
    Scene { name: "Om CVBuilder", setup: |a| a.about_open = true },
    Scene {
        name: "Koble til AI",
        setup: |a| {
            a.ai.connect_open = true;
            a.ai.choice = Some(cv_ai::Service::Claude);
        },
    },
];

fn harness(size: Vec2, setup: fn(&mut CvApp)) -> Harness<'static, CvApp> {
    let mut h = Harness::builder().with_size(size).build_eframe(move |cc| {
        let mut app = CvApp::new(cc);
        setup(&mut app);
        app
    });
    // Scrolling jumps instead of gliding, and no tooltips cover the next spot the pointer tries.
    h.ctx.all_styles_mut(|s| {
        s.scroll_animation = egui::style::ScrollAnimation::none();
        s.interaction.tooltip_delay = 1.0e6;
    });
    step(&mut h, 12);
    h
}

/// A clean portable data folder next to the test program (so nothing touches the person's own),
/// with the interface language set.
fn private_data_folder(lang: &str) {
    let dir = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("CVBuilder-data"))).expect("test exe folder");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data folder");
    std::fs::write(dir.join("settings.json"), format!("{{\"ui_language\": {lang:?}, \"remember\": false}}")).expect("settings");
}

#[test]
#[ignore = "slow: run on purpose before a release (see the module docs)"]
fn ui_qa() {
    let only = std::env::var("CVBUILDER_QA").ok();
    let langs: Vec<String> = std::env::var("CVBUILDER_QA_LANGS").map(|s| s.split(',').map(String::from).collect()).unwrap_or_else(|_| vec!["no".into(), "en".into()]);
    let sizes = [Vec2::new(1440.0, 900.0), Vec2::new(1100.0, 640.0)];
    let mut report = Vec::new();
    let mut count = 0;
    let mut baseline: HashMap<(String, u32), BTreeSet<u64>> = HashMap::new();
    for scene in SCENES.iter().filter(|s| only.as_deref().is_none_or(|o| s.name.to_lowercase().contains(&o.to_lowercase()))) {
        for lang in &langs {
            private_data_folder(lang);
            for size in sizes {
                let mut h = harness(size, scene.setup);
                let screen = Rect::from_min_size(Pos2::ZERO, size);
                let title = format!("{} · {lang} · {}×{}", scene.name, size.x, size.y);
                let base = snapshot(&h);
                // The first screen tests every control; the others what they add (a dialog's own
                // controls, a tab's tools), since the rest is the same as on the first.
                let first: BTreeSet<u64> =
                    if scene.name == "Hjem" { BTreeSet::new() } else { baseline.entry((lang.clone(), size.x as u32)).or_insert_with(|| snapshot(&harness(size, |_| {})).into_iter().map(|c| c.id).collect()).clone() };
                let ids: Vec<u64> = base.iter().filter(|c| c.control && !first.contains(&c.id)).map(|c| c.id).collect();
                count += ids.len();
                let mut problems = check_overlaps(&mut h, &base, screen);
                // Panels keep their size: one that grew has content wider than it (a row that
                // does not wrap), which pushes the other panels out of the window.
                let edges: Vec<f32> = base.iter().filter(|c| c.role == Role::Splitter).map(|c| c.rect.center().x).collect();
                if edges.first().is_some_and(|x| *x > 461.0 + 2.0) {
                    problems.push(format!("the form panel grew to {:.0} points: something in it is wider than the panel", edges[0]));
                }
                if edges.len() > 1 && edges.last().is_some_and(|x| *x < size.x - 400.0 - 2.0) {
                    problems.push(format!("the side panel grew to {:.0} points: something in it is wider than the panel", size.x - edges[edges.len() - 1]));
                }
                problems.extend(check_clicks(&mut h, &ids, screen));
                // Overlaps further down the scrolling panels too.
                for _ in 0..6 {
                    for x in [screen.width() * 0.15, screen.width() * 0.88] {
                        h.hover_at(Pos2::new(x, screen.height() * 0.6));
                        h.event(egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(0.0, -450.0),
                            modifiers: egui::Modifiers::NONE,
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Unknown,
                        });
                        step(&mut h, 2);
                    }
                    let now = snapshot(&h);
                    problems.extend(check_overlaps(&mut h, &now, screen));
                }
                // Open each dropdown and menu once, in a fresh window, and check what's inside.
                let openers: Vec<(u64, String)> = base.iter().filter(|c| opens_popup(c) && !first.contains(&c.id) && screen.intersects(c.rect)).map(|c| (c.id, c.label.clone())).collect();
                for (opener, label) in openers {
                    let mut h = harness(size, scene.setup);
                    // Into view, then a real press and release on it, like a mouse.
                    if let Some(n) = h.query_all(by().include_labels().predicate(move |n| n.locate().0.0 == opener)).next() {
                        n.scroll_to_me();
                    }
                    step(&mut h, 4);
                    let Some(at) = h.query_all(by().include_labels().predicate(move |n| n.locate().0.0 == opener)).next().map(|n| n.rect().intersect(screen).center()) else {
                        continue;
                    };
                    h.hover_at(at);
                    step(&mut h, 2);
                    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
                    step(&mut h, 1);
                    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
                    step(&mut h, 4);
                    let known: BTreeSet<u64> = base.iter().map(|c| c.id).collect();
                    let now: Vec<Ctl> = snapshot(&h).into_iter().filter(|c| !known.contains(&c.id)).collect();
                    let new: Vec<u64> = now.iter().filter(|c| c.control).map(|c| c.id).collect();
                    if new.is_empty() {
                        problems.push(format!("«{label}» opened nothing"));
                        continue;
                    }
                    count += new.len();
                    for p in check_overlaps(&mut h, &now, screen) {
                        problems.push(format!("in «{label}»: {p}"));
                    }
                    for p in check_clicks(&mut h, &new, screen) {
                        problems.push(format!("in «{label}»: {p}"));
                    }
                }
                let problems: Vec<String> = problems.into_iter().collect::<BTreeSet<_>>().into_iter().collect();
                println!("{title}: {} controls, {} problems", ids.len(), problems.len());
                for p in &problems {
                    println!("    {p}");
                }
                report.extend(problems.into_iter().map(|p| format!("{title}: {p}")));
            }
        }
    }
    i18n::set(i18n::Lang::No);
    println!("\n{count} control checks, {} problems", report.len());
    assert!(report.is_empty(), "{} problems (listed above)", report.len());
}

/// A real click on «Avbryt» closes the translate window (and doesn't fall to the window behind).
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn dialog_buttons_take_real_clicks() {
    private_data_folder("no");
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| a.open_translate(None));
    assert!(h.state().translate.is_some());
    let cancel = h.query_all(by().include_labels().predicate(|n| n.label().as_deref() == Some("Avbryt"))).next().map(|n| n.rect()).expect("Avbryt");
    h.hover_at(cancel.center());
    step(&mut h, 2);
    h.event(egui::Event::PointerButton { pos: cancel.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    step(&mut h, 1);
    h.event(egui::Event::PointerButton { pos: cancel.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    step(&mut h, 3);
    assert!(h.state().translate.is_none(), "the click on «Avbryt» did not close the window");
}

/// Pictures of every scene, for looking at the layout by eye:
/// `CVBUILDER_SHOTS=<dir> cargo test --release -p cvbuilder-app ui_shots -- --ignored`
#[test]
#[ignore = "writes pictures; run on purpose"]
fn ui_shots() {
    let Ok(dir) = std::env::var("CVBUILDER_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("shots folder");
    let lang = std::env::var("CVBUILDER_QA_LANGS").unwrap_or_else(|_| "no".into());
    private_data_folder(&lang);
    for (i, scene) in SCENES.iter().enumerate() {
        let mut h = harness(Vec2::new(1440.0, 900.0), scene.setup);
        let image = h.render().expect("render");
        image.save(dir.join(format!("{i:02}-{}.png", scene.name.replace(' ', "_")))).expect("save");
    }
}

/// Store screenshots: the example CV with its preview drawn, at 1920×1080.
/// `CVBUILDER_SHOTS=<dir> CVBUILDER_QA_LANGS=en cargo test --release -p cvbuilder-app store_shots -- --ignored`
#[test]
#[ignore = "writes pictures; run on purpose"]
fn store_shots() {
    let Ok(dir) = std::env::var("CVBUILDER_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("shots folder");
    let lang = std::env::var("CVBUILDER_QA_LANGS").unwrap_or_else(|_| "no".into());
    private_data_folder(&lang);
    let scenes: [(&str, fn(&mut CvApp)); 4] = [
        ("1-skjema-og-forhandsvisning", |_| {}),
        ("2-sjekkliste", |a| a.side = Side::Checks),
        ("3-design", |a| a.ribbon_tab = Tab::Design),
        ("4-sideoppsett", |a| a.ribbon_tab = Tab::Page),
    ];
    for (name, setup) in scenes {
        let mut h = harness(Vec2::new(1920.0, 1080.0), setup);
        // The preview is made on its own thread after a short pause: wait for the pages.
        for _ in 0..200 {
            h.step();
            if h.state().preview.page_count > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        step(&mut h, 20);
        let image = h.render().expect("render");
        image.save(dir.join(format!("{lang}-{name}.png"))).expect("save");
    }
}
