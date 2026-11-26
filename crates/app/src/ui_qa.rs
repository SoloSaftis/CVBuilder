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
    c.control && (c.role == Role::ComboBox || caret || MENUS.iter().any(|m| label.strip_prefix(m).is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))))
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
    Scene {
        name: "Hva et ATS leser",
        setup: |a| {
            a.side = Side::Checks;
            a.project.options.layout = "rr-pikachu-2".into();
            let result = crate::readback_ui::run(&a.project, false);
            a.readback.show(result, "Pikachu · PDF");
        },
    },
    Scene { name: "Søknadsskjema", setup: |a| a.side = Side::Form },
    // 051: the interview prep sheet over the example CV and its ad, with AI (the practise button)
    // and without, and for an empty CV (only the usual questions).
    Scene { name: "Intervjuark", setup: |a| a.interview.open() },
    Scene {
        name: "Intervjuark med AI",
        setup: |a| {
            with_provider(a);
            a.interview.open();
        },
    },
    Scene {
        // Nothing to ask about yet but the usual questions. The example CV with no bullets, no ad
        // and no keywords, so the form behind the window has no controls the baseline lacks.
        name: "Intervjuark uten innhold",
        setup: |a| {
            for job in &mut a.project.experience {
                job.bullets.clear();
            }
            a.project.custom_sections.clear();
            a.project.target.keywords.clear();
            a.project.target.description.clear();
            a.interview.open();
        },
    },
    // Possible spelling mistakes with suggestions, ignore and «my words» (046). The plain
    // «Sjekkliste» scene shows the card for a Norwegian CV without a dictionary: the download button.
    Scene {
        name: "Stavekontroll",
        setup: |a| {
            use cv_core::spelling::{Finding, Place};
            a.side = Side::Checks;
            let job = Place::Job(a.project.experience.first().map(|j| j.title.clone()).unwrap_or_default());
            let found = |word: &str, count: usize, places: Vec<Place>, s: &[&str]| (Finding { word: word.into(), count, places }, s.iter().map(|x| x.to_string()).collect());
            a.spelling.own.insert("kundeløftet".into());
            a.spelling.show(
                a.project.language_code(),
                vec![
                    found("ansvarlihg", 2, vec![Place::Profile, job.clone()], &["ansvarlig", "ansvarlige"]),
                    found("Kundeorientertt", 1, vec![job], &["Kundeorientert"]),
                    found("leveranseprosjektledelsesansvar", 1, vec![Place::Section("Prosjekter".into())], &[]),
                ],
            );
        },
    },
    // Must-haves and nice-to-haves read from the ad, with where the CV shows them (045).
    Scene {
        name: "Krav i annonsen",
        setup: |a| {
            a.side = Side::Checks;
            a.project.target.description = include_str!("../../cv-core/tests/ads/elektriker-no.txt").into();
            a.project.target.keywords = vec!["fagbrev".into(), "KNX".into(), "NEK 400".into()];
        },
    },
    Scene {
        name: "Krav i engelsk annonse",
        setup: |a| {
            a.side = Side::Checks;
            a.project.target.description = include_str!("../../cv-core/tests/ads/data-engineer-en.txt").into();
        },
    },
    // An ad with no headings: the hint and the AI button.
    Scene {
        name: "Krav i annonse uten overskrifter",
        setup: |a| {
            with_provider(a);
            a.side = Side::Checks;
            a.project.target.description = "Vi søker en blid medarbeider til butikken vår. Send søknad i dag!".into();
        },
    },
    Scene {
        name: "Egen sjekkliste",
        setup: |a| {
            a.side = Side::Checks;
            a.settings.own_checklist = true;
            a.checklist.own = Some(cv_core::checklist::Checklist::starter());
        },
    },
    Scene { name: "Alle seksjoner åpne", setup: |a| a.folded.clear() },
    // A tailored copy (041): a job, a bullet, an education, a skill and a language hidden, the
    // bullet list open, the banner at the top and the count in the status bar.
    Scene {
        name: "Skjulte deler",
        setup: |a| {
            a.folded.clear();
            let p = &mut a.project;
            p.options.copy_of = "Ingrid_Haugland_CV_utkast.json".into();
            let ids = [p.experience[1].id.clone(), p.experience[0].bullet_ids[1].clone(), p.education[0].id.clone(), p.skills[0].id.clone(), p.languages[0].id.clone()];
            for id in ids {
                p.set_hidden(&id, true);
            }
            a.preview.changed();
        },
    },
    // A CV without a job ad: the ad card is one line and the contact details come first (028).
    Scene { name: "Tom CV", setup: |a| a.load(cv_core::Project::default(), false) },
    Scene {
        name: "Tom CV med annonsefeltene",
        setup: |a| {
            a.load(cv_core::Project::default(), false);
            a.ad_open = true;
        },
    },
    Scene { name: "Oversett-vinduet", setup: |a| a.open_translate(None) },
    Scene { name: "Innstillinger", setup: |a| a.settings_open = true },
    // The translators' tools unfolded, and the draft not remembered (the hint about the risk).
    Scene {
        name: "Innstillinger med språkverktøy",
        setup: |a| {
            a.settings_open = true;
            a.settings.remember = false;
            a.langs.tools_open = true;
        },
    },
    Scene { name: "Ny seksjon", setup: |a| a.new_section = Some(String::new()) },
    Scene { name: "Sjekklisteredigering", setup: |a| a.checklist.editing = Some(cv_core::checklist::Checklist::starter()) },
    Scene { name: "Om CVBuilder", setup: |a| a.about_open = true },
    Scene { name: "Importer design", setup: |a| a.designs.import_open = true },
    Scene {
        name: "Oppdatering finnes",
        setup: |a| a.update_check = Some(crate::updates_ui::UpdateCheck::Done(Ok(cv_ai::updates::Release { version: "9.9.9".into(), page: cv_ai::updates::RELEASES_PAGE.into() }))),
    },
    Scene { name: "Sjekk feilet", setup: |a| a.update_check = Some(crate::updates_ui::UpdateCheck::Done(Err(cv_ai::updates::failure(Some(503), "GitHub answered 503")))) },
    // A save that failed: the plain message in the status bar, the technical text on hover.
    Scene {
        name: "Lagring feilet",
        setup: |a| {
            a.status = crate::i18n::failed(crate::i18n::t("Utkastet ble ikke lagret."), "Access is denied. (os error 5)", cv_core::plain::TRY_ANOTHER_PLACE);
        },
    },
    Scene {
        name: "Lagre før lukking",
        setup: |a| {
            a.is_example = false;
            a.leaving = Some(crate::Leave::Close);
        },
    },
    Scene { name: "Velkommen", setup: |a| a.welcome = Some(crate::welcome_ui::Welcome::for_this_computer(&a.langs.packs)) },
    Scene { name: "Importer CV", setup: |a| a.import.cv_open = true },
    // 048: a Europass CV read field by field fills the form (contact lines, driving licence,
    // CEFR levels). The window with what has no place is the «Importrest» scene.
    Scene {
        name: "Europass hentet inn",
        setup: |a| {
            let read = cv_core::europass::read_xml(include_str!("../../cv-core/tests/europass/skillspassport-no.xml")).expect("fixture");
            a.apply_read(read, "Hentet inn fra Europass-CV-en: {found}. Den forrige CV-en kan hentes tilbake med Angre.");
            a.import.rest = None;
        },
    },
    // 031: Undo and Redo both have a named step, and the status line names what was undone.
    Scene {
        name: "Angre-steg",
        setup: |a| {
            a.replace(cv_core::Project::default(), false, crate::step::NEW, "");
            a.replace(cv_core::Project::example(), false, crate::step::IMPORT, "");
            a.remember(crate::step::DELETE_JOB);
            a.undo();
        },
    },
    Scene { name: "Importrest", setup: |a| a.import.rest = Some("ERFARING
Som koordinator i Fjellvegg Bygg AS 2021 – nå".into()) },
    Scene {
        name: "Koble til AI",
        setup: |a| {
            a.ai.connect_open = true;
            a.ai.choice = Some(cv_ai::Service::Claude);
        },
    },
    // Where the CV goes and what it costs (to-do 030).
    Scene { name: "Før AI-en starter", setup: |a| {
        with_provider(a);
        a.ask_ai(cv_ai::cost::Work::Tailor, "Tilpass CV-en til stillingsannonsen.");
    } },
    Scene { name: "Oversett med AI-tjeneste", setup: |a| {
        with_provider(a);
        a.open_translate(None);
    } },
    Scene { name: "Oversett med lokal modell", setup: |a| {
        with_local(a);
        a.open_translate(None);
    } },
    Scene { name: "Importer CV med AI", setup: |a| {
        with_provider(a);
        a.import.cv_open = true;
        a.import.use_ai = true;
    } },
    Scene { name: "Importrest med AI", setup: |a| {
        with_provider(a);
        a.import.rest = Some("ERFARING\nSom koordinator i Fjellvegg Bygg AS 2021 – nå".into());
    } },
    Scene { name: "AI-panel med nøkkel", setup: |a| {
        with_provider(a);
        a.side = Side::Chat;
    } },
    Scene { name: "Koble til Claude Code", setup: |a| {
        a.chat.claude_code = Some("claude".into());
        a.ai.connect_open = true;
        a.ai.claude_code_tab = true;
    } },
    Scene {
        name: "Koble til lokal AI",
        setup: |a| {
            a.ai.connect_open = true;
            a.ai.choice = Some(cv_ai::Service::Local);
        },
    },
    // The AI panel with an AI connected: the intro above the suggestions, whole and wrapped.
    Scene {
        name: "AI-panelet tilkoblet",
        setup: |a| {
            a.settings.ai_service = Some(cv_ai::Service::Local);
            a.settings.ai_model = "qwen3".into();
            a.settings.chat_backend = "api".into();
        },
    },
    Scene { name: "Bruk fra andre AI-apper", setup: |a| a.chat.mcp_help_open = true },
];

/// A cloud AI connected with a key kept only in memory (never the keychain), and the API chosen.
fn with_provider(a: &mut CvApp) {
    a.ai.keys.keep_for_session(cv_ai::Service::ChatGpt, "sk-test-not-a-real-key");
    a.settings.ai_service = Some(cv_ai::Service::ChatGpt);
    a.settings.ai_model = "gpt-5".into();
    a.settings.chat_backend = "api".into();
}

/// A model on this computer (Ollama's address), with no key.
fn with_local(a: &mut CvApp) {
    a.settings.ai_service = Some(cv_ai::Service::Local);
    a.settings.local_url = "http://localhost:11434/v1".into();
    a.settings.ai_model = "qwen3".into();
    a.settings.chat_backend = "api".into();
}

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
    wait_for_preview(&mut h);
    h
}

/// The pages are drawn on another thread, and the preview only gets its scroll bar once they are
/// there. Every window waits for them, so that none is compared half loaded: a scroll bar that
/// turned up behind a dialog looked like part of the dropdown just opened there.
fn wait_for_preview(h: &mut Harness<'_, CvApp>) {
    let start = std::time::Instant::now();
    while h.state().preview.pending() {
        assert!(start.elapsed().as_secs() < 60, "the preview was never drawn");
        std::thread::sleep(std::time::Duration::from_millis(10));
        h.step();
    }
    // The pages take their place, and the scroll bar with them.
    step(h, 20);
}

/// The harness hands the window over with its pages drawn, so no control turns up later by itself.
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn nothing_appears_after_the_window_is_ready() {
    private_data_folder("en");
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| a.open_translate(None));
    assert!(!h.state().preview.textures.is_empty(), "no pages yet");
    let ids = |h: &Harness<'_, CvApp>| snapshot(h).into_iter().filter(|c| c.control).map(|c| c.id).collect::<BTreeSet<_>>();
    let before = ids(&h);
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    assert_eq!(before, ids(&h));
}

/// A clean portable data folder next to the test program (so nothing touches the person's own),
/// with the interface language set.
fn private_data_folder(lang: &str) {
    let dir = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("CVBuilder-data"))).expect("test exe folder");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("data folder");
    // The CV language is set too, so the example (Norwegian for «no», English otherwise) is the
    // same on every computer, whatever its own language.
    let cv = if lang == "no" { "Bokmål" } else { "English" };
    std::fs::write(dir.join("settings.json"), format!("{{\"ui_language\": {lang:?}, \"cv_language\": {cv:?}, \"remember\": false, \"welcomed\": true}}")).expect("settings");
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
                // controls, a tab's tools, a button greyed out there), since the rest is the same
                // as on the first.
                let first: BTreeSet<u64> = if scene.name == "Hjem" {
                    BTreeSet::new()
                } else {
                    baseline.entry((lang.clone(), size.x as u32)).or_insert_with(|| snapshot(&harness(size, |_| {})).into_iter().filter(|c| c.control).map(|c| c.id).collect()).clone()
                };
                // The panels' scroll bars are egui's own and are not click-tested here; a dropdown's
                // or a menu's are, with the rest of what it opens.
                let ids: Vec<u64> = base.iter().filter(|c| c.control && c.role != Role::ScrollBar && !first.contains(&c.id)).map(|c| c.id).collect();
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

/// A real click in the middle of the control with this label.
fn click(h: &mut Harness<'_, CvApp>, label: &str) {
    let at = h.query_all(by().include_labels().predicate(|n| n.label().as_deref() == Some(label))).last().map(|n| n.rect().center()).unwrap_or_else(|| panic!("no «{label}»"));
    h.hover_at(at);
    step(h, 2);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    step(h, 1);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    step(h, 3);
}

/// Every button on the main screens has a name a screen reader can say: icon buttons, drawn
/// buttons and clickable areas get their hover text as their name (theme::named).
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn every_button_has_a_name() {
    private_data_folder("en");
    let mut nameless = Vec::new();
    for scene in SCENES.iter().filter(|s| ["Hjem", "Alle seksjoner åpne", "Sideoppsett", "Design", "Sjekkliste"].contains(&s.name)) {
        let h = harness(Vec2::new(1440.0, 900.0), scene.setup);
        let named_by_content = |r: Role| matches!(r, Role::TextInput | Role::MultilineTextInput | Role::ScrollBar | Role::Splitter);
        for c in snapshot(&h).into_iter().filter(|c| c.control && !named_by_content(c.role)) {
            let label = c.label;
            // An icon-font character alone (Private Use Area) is no name either.
            if label.trim().chars().all(|c| ('\u{e000}'..='\u{f8ff}').contains(&c)) {
                nameless.push(format!("{}: a {:?} at {:?}", scene.name, c.role, c.rect.center()));
            }
        }
    }
    assert!(nameless.is_empty(), "buttons without a name:\n{}", nameless.join("\n"));
    // Drawn buttons without a name are not in the accessibility tree at all, so ask for them by
    // name: the up, down and delete buttons of an open job.
    let h = harness(Vec2::new(1440.0, 900.0), |a| a.folded.clear());
    // …and as buttons, so a screen reader says what they are, not only what they are called.
    let names: Vec<String> = snapshot(&h).into_iter().filter(|c| c.control && c.role == Role::Button).map(|c| c.label).collect();
    for wanted in ["Move up in the CV", "Move down in the CV", "Delete the job"] {
        assert!(names.iter().any(|n| n == wanted), "no button named «{wanted}» (a drawn button lost its name or its button role)");
    }
}

/// Every point in the checklist tab tells a screen reader its status in words before the text
/// («To do: …»), in the standard check and in one's own, so the status is never colour alone
/// (029). The to-do points of the recruiter check come before the ones that are done.
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn checklist_points_say_their_status() {
    use crate::checklist_ui::Status;
    private_data_folder("en");
    i18n::set(i18n::Lang::En);
    let words: Vec<String> = Status::ALL.iter().map(|s| format!("{}: ", s.word())).collect();
    let spoken = |h: &Harness<'_, CvApp>| -> Vec<String> { snapshot(h).into_iter().map(|c| c.label).filter(|l| words.iter().any(|w| l.starts_with(w.as_str()))).collect() };
    let scene = |name: &str| SCENES.iter().find(|s| s.name == name).expect("scene").setup;
    // The example passes nearly every point; without a headline one is left to do.
    let h = harness(Vec2::new(1440.0, 900.0), |a| {
        a.side = Side::Checks;
        a.project.candidate.headline.clear();
    });
    let points = spoken(&h);
    assert!(points.iter().any(|l| l.starts_with("To do: ")), "no point says «To do»:\n{}", points.join("\n"));
    assert!(points.iter().any(|l| l.starts_with("Done: ")), "no point says «Done»:\n{}", points.join("\n"));
    // In the recruiter check (after «Before download»), nothing done comes before a point to do.
    let recruiter: Vec<&String> = points.iter().skip_while(|l| !l.starts_with("Done: Everything required")).skip(1).collect();
    let first_done = recruiter.iter().position(|l| l.starts_with("Done: ")).unwrap_or(recruiter.len());
    assert!(recruiter[first_done..].iter().all(|l| l.starts_with("Done: ")), "a point to do comes after a done one:\n{}", points.join("\n"));
    // One's own list.
    let h = harness(Vec2::new(1440.0, 900.0), scene("Egen sjekkliste"));
    assert!(!spoken(&h).is_empty(), "the own checklist has no spoken points");
    let labels: Vec<String> = snapshot(&h).into_iter().map(|c| c.label).collect();
    // The dropdown is named for what it does (029); with one's own list chosen, that list shows.
    assert!(labels.iter().any(|l| l.starts_with("Which checklist you see")), "the checklist dropdown has no name:\n{}", labels.join("\n"));
    assert!(labels.iter().any(|l| l.ends_with("points are met.")), "one's own list is not shown:\n{}", labels.join("\n"));
}

/// 048: the File menu offers the JSON Resume export.
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn file_menu_exports_json_resume() {
    private_data_folder("no");
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| a.settings.ui_language = "no".into());
    click(&mut h, "Fil");
    let labels: Vec<String> = snapshot(&h).into_iter().map(|c| c.label).collect();
    assert!(labels.iter().any(|l| l.contains("Eksporter som JSON Resume")), "no JSON Resume export in the File menu:
{}", labels.join("
"));
}

/// The welcome window's three ways to start each do what they say, and the answer is kept.
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn welcome_asks_how_to_start() {
    let welcome = |a: &mut CvApp| {
        a.settings.ui_language = "no".into();
        a.welcome = Some(crate::welcome_ui::Welcome::for_this_computer(&a.langs.packs));
    };
    // Import: the import window opens.
    private_data_folder("no");
    let mut h = harness(Vec2::new(1440.0, 900.0), welcome);
    click(&mut h, "Hent inn CV-en jeg har");
    assert!(h.state().welcome.is_none() && h.state().settings.welcomed);
    assert!(h.state().import.cv_open, "the import window did not open");
    // Empty: a blank CV, not the example.
    private_data_folder("no");
    let mut h = harness(Vec2::new(1440.0, 900.0), welcome);
    click(&mut h, "Start med en tom CV");
    assert!(!h.state().is_example && h.state().project.experience.is_empty(), "no empty CV");
    // Example: the example stays.
    private_data_folder("no");
    let mut h = harness(Vec2::new(1440.0, 900.0), welcome);
    click(&mut h, "Se et eksempel først");
    assert!(h.state().welcome.is_none() && h.state().is_example);
}

/// A CV without a job ad starts with the name: the ad card is one line until it is clicked, and
/// the example (which has an ad) shows the whole card. Download PDF has its own group, last on
/// Home, and can be seen without scrolling the ribbon (028).
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn form_starts_with_the_name_without_an_ad() {
    let line = "Søker du på en bestemt jobb? Lim inn annonsen her (valgfritt)";
    private_data_folder("no");
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| a.load(cv_core::Project::default(), false));
    assert!(has_label(&h, line), "no one-line job-ad card on an empty CV");
    assert!(!has_label(&h, "Lenke til annonsen"), "the ad fields show on an empty CV");
    assert!(has_label(&h, "Start her: navnet ditt og hvordan en arbeidsgiver når deg."), "no «start here» on the contact details");
    // Above the name, and the name is in the first screen of the form.
    let top = |h: &Harness<'_, CvApp>, l: &str| h.query_all(by().include_labels().predicate(|n| n.label().as_deref() == Some(l) || n.value().as_deref() == Some(l))).next().map(|n| n.rect().min.y).unwrap_or_else(|| panic!("no «{l}»"));
    let name = top(&h, "Fullt navn");
    assert!(top(&h, line) < name && name < 400.0, "the name is not near the top of the form ({name})");
    click(&mut h, line);
    assert!(h.state().ad_open && has_label(&h, "Lenke til annonsen"), "a click on the line did not open the ad card");
    // Folded while still empty: back to one line.
    click(&mut h, "Klikk for å skjule annonsefeltene");
    assert!(has_label(&h, line), "folding the empty ad card did not make it one line again");
    // «Lim inn annonse» on the ribbon opens it too.
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| a.load(cv_core::Project::default(), false));
    let ctx = h.ctx.clone();
    h.state_mut().run_action(&ctx, crate::menu_ui::Action::PasteAd);
    step(&mut h, 4);
    assert!(has_label(&h, "Lenke til annonsen"), "«Lim inn annonse» did not open the ad card");
    // The example has an ad: the whole card.
    let h = harness(Vec2::new(1440.0, 900.0), |_| {});
    assert!(has_label(&h, "Lenke til annonsen") && !has_label(&h, line), "the example's ad card is not shown in full");
    // Download PDF: in view in every language and window size the screen check uses.
    let langs: Vec<String> = std::env::var("CVBUILDER_QA_LANGS").map(|s| s.split(',').map(String::from).collect()).unwrap_or_else(|_| vec!["no".into(), "en".into()]);
    for lang in &langs {
        private_data_folder(lang);
        for size in [Vec2::new(1440.0, 900.0), Vec2::new(1100.0, 640.0)] {
            let h = harness(size, |_| {});
            let pdf = i18n::t("Last ned PDF");
            let r = h.query_all(by().include_labels().predicate(|n| n.role() == Role::Button && n.label().is_some_and(|l| l.ends_with(pdf)))).next().map(|n| n.rect()).unwrap_or_else(|| panic!("{lang}: no «{pdf}» button"));
            println!("{lang} {}: «{pdf}» at {:.0}–{:.0}", size.x, r.min.x, r.max.x);
            assert!(r.max.x <= size.x - 8.0, "{lang} at {}: «{pdf}» is outside the window ({:.0})", size.x, r.max.x);
        }
    }
    i18n::set(i18n::Lang::No);
}

/// The AI panel's intro is read whole: it sits between the header and the first suggestion,
/// inside the window, in a long language and a small window too. The header has no «MCP».
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn ai_intro_is_whole() {
    let connected = |a: &mut CvApp| {
        a.settings.ai_service = Some(cv_ai::Service::Local);
        a.settings.ai_model = "qwen3".into();
        a.settings.chat_backend = "api".into();
    };
    for lang in ["no", "de", "ta"] {
        for size in [Vec2::new(1440.0, 900.0), Vec2::new(1100.0, 640.0)] {
            private_data_folder(lang);
            let h = harness(size, connected);
            let ppp = h.ctx.pixels_per_point();
            let rect = |text: &'static str| {
                h.query_all(by().include_labels().predicate(move |n| n.label().or_else(|| n.value()).as_deref() == Some(i18n::t(text))))
                    .find_map(|n| n.accesskit_node().bounding_box())
                    .map(|b| Rect::from_min_max(Pos2::new(b.x0 as f32 / ppp, b.y0 as f32 / ppp), Pos2::new(b.x1 as f32 / ppp, b.y1 as f32 / ppp)))
            };
            let header = rect("Du snakker med").expect("header");
            let intro = rect("AI-en endrer CV-en direkte. Du kan angre hver endring. Den finner aldri på fakta: mangler et tall, skriver den [tall] som du fyller inn.").expect("intro");
            let first = rect("Skreddersy CV-en til stillingsannonsen").expect("first suggestion");
            let at = format!("{lang} {}×{}", size.x, size.y);
            assert!(intro.top() > header.bottom() && intro.bottom() <= first.top() + 1.0, "{at}: the intro {intro:?} is not between the header {header:?} and the suggestions {first:?}");
            assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(intro), "{at}: the intro {intro:?} runs out of the window");
            assert!(!has_label(&h, "MCP"), "{at}: «MCP» is still in the AI panel");
        }
    }
    i18n::set(i18n::Lang::No);
}

/// Opens the dropdown that now shows `value`, with a real click.
fn open_dropdown(h: &mut Harness<'_, CvApp>, value: &str) {
    let at = h
        .query_all(by().include_labels().predicate(|n| n.role() == Role::ComboBox && n.value().as_deref() == Some(value)))
        .last()
        .map(|n| n.rect().center())
        .unwrap_or_else(|| panic!("no dropdown showing «{value}»"));
    h.hover_at(at);
    step(h, 2);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    step(h, 1);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    step(h, 4);
}

/// Types into whatever has the keyboard, then steps.
fn type_text(h: &mut Harness<'_, CvApp>, text: &str) {
    h.event(egui::Event::Text(text.into()));
    step(h, 3);
}

fn press(h: &mut Harness<'_, CvApp>, key: egui::Key) {
    h.key_press(key);
    step(h, 3);
}

/// The texts of the items now shown in an open language list (selectable lines).
fn has_label(h: &Harness<'_, CvApp>, label: &str) -> bool {
    h.query_all(by().include_labels().predicate(|n| n.label().as_deref() == Some(label) || n.value().as_deref() == Some(label))).next().is_some()
}

/// Every language list has a search field that has the keyboard as soon as the list opens:
/// typing filters by the name in the menu language, the own name or the code, and Enter picks.
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn language_lists_search() {
    use crate::translate_ui::shown_name;
    private_data_folder("no");

    // Home: the CV language. The own name finds it, and the list shows only what fits.
    let mut h = harness(Vec2::new(1440.0, 900.0), |_| {});
    open_dropdown(&mut h, &shown_name("Bokmål"));
    assert!(has_label(&h, &shown_name("Français")), "the list did not open with every language");
    type_text(&mut h, "Deutsch");
    assert!(has_label(&h, &shown_name("Deutsch")), "German is not in the list after typing «Deutsch»");
    assert!(!has_label(&h, &shown_name("Français")), "French is still in the list after typing «Deutsch»");
    // The arrows move the mark and Enter picks the marked line.
    press(&mut h, egui::Key::ArrowDown);
    press(&mut h, egui::Key::ArrowUp);
    press(&mut h, egui::Key::Enter);
    assert_eq!(h.state().project.options.headings.as_ref().map(|x| x.language.as_str()), Some("Deutsch"), "Enter did not pick German");
    // Closed: an open list would show every language again (the search starts afresh).
    assert!(!has_label(&h, &shown_name("Français")), "the list stayed open after Enter");

    // Nothing fits: the list says so.
    let mut h = harness(Vec2::new(1440.0, 900.0), |_| {});
    open_dropdown(&mut h, &shown_name("Bokmål"));
    type_text(&mut h, "qqqq");
    assert!(has_label(&h, i18n::t("Fant ikke språket")), "no «not found» line");
    press(&mut h, egui::Key::Enter);
    assert!(h.state().project.options.headings.is_none(), "Enter with no match changed the CV language");

    // The translate window: the name in the menu language.
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| a.open_translate(None));
    let now = shown_name(&h.state().translate.as_ref().expect("translate window").language);
    open_dropdown(&mut h, &now);
    type_text(&mut h, &shown_name("Français"));
    press(&mut h, egui::Key::Enter);
    assert_eq!(h.state().translate.as_ref().map(|t| t.language.as_str()), Some("Français"));

    // The welcome window: the code finds the CV language.
    let welcome = |a: &mut CvApp| {
        a.settings.ui_language = "no".into();
        a.welcome = Some(crate::welcome_ui::Welcome::for_this_computer(&a.langs.packs));
    };
    let mut h = harness(Vec2::new(1440.0, 900.0), welcome);
    let now = shown_name(&h.state().welcome.as_ref().expect("welcome").cv_language);
    open_dropdown(&mut h, &now);
    type_text(&mut h, "pl");
    press(&mut h, egui::Key::Enter);
    assert_eq!(h.state().welcome.as_ref().map(|w| w.cv_language.as_str()), Some("Polski"));

    // Settings: the menu language, by its English name.
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| a.settings_open = true);
    let items = crate::langpack_ui::ui_language_items(&h.state().langs.packs);
    let now = items.iter().find(|it| it.key == "no").map(|it| it.shown.clone()).expect("Norwegian");
    open_dropdown(&mut h, &now);
    type_text(&mut h, "Swedish");
    press(&mut h, egui::Key::Enter);
    assert_eq!(h.state().settings.ui_language, "sv");
    i18n::set(i18n::Lang::No);
}

/// The settings window: the four groups are there, the translators' tools are folded until
/// asked for, and the fold opens and closes from the keyboard too.
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn settings_groups_and_language_tools() {
    private_data_folder("no");
    i18n::set(i18n::Lang::No);
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| a.settings_open = true);
    for group in ["Språk", "Utseende", "Lagring", "AI og data"] {
        assert!(has_label(&h, group), "no group «{group}»");
    }
    assert!(!has_label(&h, "Eksporter mal for oversettelse …"), "the language tools are not folded");
    click(&mut h, "Språkverktøy");
    assert!(h.state().langs.tools_open && has_label(&h, "Eksporter mal for oversettelse …"), "«Språkverktøy» did not unfold");
    // Norwegian has no pack: the buttons for an added language are hidden, with a hint saying why.
    assert!(!has_label(&h, "Rediger …"));
    // From the keyboard: Tab reaches the fold's header, and Space folds it again.
    let at = h.query_all(by().include_labels().predicate(|n| n.label().as_deref() == Some("Språkverktøy"))).last().map(|n| n.rect().center()).expect("header");
    let mut found = false;
    for _ in 0..40 {
        press(&mut h, egui::Key::Tab);
        let focused = h.ctx.memory(|m| m.focused()).and_then(|id| h.ctx.read_response(id));
        if focused.is_some_and(|r| r.rect.contains(at)) {
            found = true;
            break;
        }
    }
    assert!(found, "Tab never reached «Språkverktøy»");
    press(&mut h, egui::Key::Space);
    assert!(!h.state().langs.tools_open, "Space on «Språkverktøy» did not fold it");
}

/// Saving like Word: Ctrl+S into the CV's file, the question on close, the autosave every five
/// minutes, and the file coming back after a restart.
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn save_close_and_autosave() {
    private_data_folder("no");
    let file = std::env::temp_dir().join(format!("cvbuilder-save-test-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let on_disk = |f: &std::path::Path| cv_core::Project::from_json(&std::fs::read_to_string(f).expect("the CV file")).expect("a CV");

    // The example has nothing to save; an edit makes it the person's own, unsaved CV.
    let mut h = harness(Vec2::new(1440.0, 900.0), |_| {});
    assert!(!h.state().unsaved(), "the example asks to be saved");
    h.state_mut().project.candidate.name = "Kari Test".into();
    h.state_mut().touched();
    step(&mut h, 2);
    assert!(h.state().unsaved());

    // The window's close button asks first; «Avbryt» keeps the window and the changes.
    h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
    h.step();
    assert!(h.state().leaving.is_some(), "closing with unsaved changes did not ask");
    step(&mut h, 2);
    click(&mut h, "Avbryt");
    assert!(h.state().leaving.is_none() && !h.state().may_close, "«Avbryt» did not keep the window open");
    h.state_mut().leaving = Some(crate::Leave::Close);
    step(&mut h, 2);
    click(&mut h, "Lukk uten å lagre");
    assert!(h.state().leaving.is_none() && h.state().may_close, "«Lukk uten å lagre» did not let the window close");

    // «Lagre» in the question writes the CV's file and then lets the window close.
    h.state_mut().may_close = false;
    h.state_mut().file = Some(file.clone());
    h.state_mut().leaving = Some(crate::Leave::Close);
    step(&mut h, 2);
    click(&mut h, "Lagre");
    assert!(h.state().may_close, "«Lagre» did not close after saving");
    assert_eq!(on_disk(&file).candidate.name, "Kari Test");
    assert!(!h.state().unsaved());

    // Ctrl+S saves straight into the same file.
    h.state_mut().may_close = false;
    h.state_mut().project.candidate.name = "Kari Ctrl".into();
    h.state_mut().touched();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    step(&mut h, 3);
    assert_eq!(on_disk(&file).candidate.name, "Kari Ctrl", "Ctrl+S did not save");
    assert!(!h.state().may_close, "Ctrl+S closed the window");

    // Nothing is written before five minutes have gone; after that, the changes are.
    h.state_mut().project.candidate.name = "Kari Auto".into();
    h.state_mut().touched();
    step(&mut h, 3);
    assert_eq!(on_disk(&file).candidate.name, "Kari Ctrl", "autosaved too soon");
    h.state_mut().last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(5 * 60 + 1);
    step(&mut h, 3);
    assert_eq!(on_disk(&file).candidate.name, "Kari Auto", "no autosave after five minutes");
    assert!(h.state().status.contains("automatisk"), "status: {}", h.state().status);

    // Turned off in the settings: no autosave.
    h.state_mut().settings.autosave = false;
    h.state_mut().project.candidate.name = "Kari Av".into();
    h.state_mut().touched();
    h.state_mut().last_autosave = std::time::Instant::now() - std::time::Duration::from_secs(10 * 60);
    step(&mut h, 3);
    assert_eq!(on_disk(&file).candidate.name, "Kari Auto", "autosaved while turned off");

    // A new CV forgets the file, so «Lagre» cannot write it over the old one.
    h.state_mut().replace(cv_core::Project::example(), false, crate::step::EXAMPLE, "");
    assert!(h.state().file.is_none(), "a new CV kept the old CV's file");

    // After a restart the draft comes back with its file, and nothing is unsaved.
    h.state_mut().settings.autosave = true;
    h.state_mut().settings.remember = true;
    h.state_mut().project.candidate.name = "Kari Auto".into();
    h.state_mut().set_file(Some(file.clone()));
    h.state_mut().persist();
    drop(h);
    let mut h = harness(Vec2::new(1440.0, 900.0), |_| {});
    step(&mut h, 2);
    assert_eq!(h.state().file.as_deref(), Some(file.as_path()), "the file was not remembered");
    assert!(!h.state().unsaved(), "a fresh start counts as unsaved");
    let _ = std::fs::remove_file(&file);
}

/// The keyboard: Ctrl+N, Ctrl+Z and Ctrl+Y (Cmd on the Mac) work on the whole CV, but a text field
/// keeps its own Ctrl+Z; a dialog has the keyboard while it is open; Enter presses the dialog's
/// safe default and Esc closes it; the language editor does not close and lose edits on Esc or a
/// click beside it.
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn keyboard_shortcuts_and_safe_dialogs() {
    let cmd = egui::Modifiers::COMMAND;
    let cmd_shift = egui::Modifiers { shift: true, ..egui::Modifiers::COMMAND };
    private_data_folder("no");
    let mut h = harness(Vec2::new(1440.0, 900.0), |_| {});
    assert!(h.state().is_example);

    // Ctrl+N: a blank CV; Ctrl+Z brings the example back; Ctrl+Y and Ctrl+Shift+Z go forward again.
    h.key_press_modifiers(cmd, egui::Key::N);
    step(&mut h, 3);
    assert!(!h.state().is_example && h.state().project.candidate.name.is_empty(), "Ctrl+N made no new CV");
    // The Undo button says what it undoes, and so does the status line after (031).
    assert_eq!(h.state().undo_tip(), "Angre: ny CV");
    h.key_press_modifiers(cmd, egui::Key::Z);
    step(&mut h, 3);
    assert!(h.state().is_example, "Ctrl+Z did not undo the new CV");
    assert!(h.state().status.starts_with("Angret: ny CV."), "status: {}", h.state().status);
    assert_eq!(h.state().redo_tip(), "Gjør om: ny CV");
    h.key_press_modifiers(cmd, egui::Key::Y);
    step(&mut h, 3);
    assert!(!h.state().is_example, "Ctrl+Y did not redo");
    h.key_press_modifiers(cmd, egui::Key::Z);
    step(&mut h, 3);
    h.key_press_modifiers(cmd_shift, egui::Key::Z);
    step(&mut h, 3);
    assert!(!h.state().is_example, "Ctrl+Shift+Z did not redo");
    assert!(!h.state().history.can_redo() && h.state().history.can_undo());

    // In a text field Ctrl+Z is the field's: the CV stays.
    let field = snapshot(&h).into_iter().find(|c| c.role == Role::TextInput).expect("a text field").rect;
    h.hover_at(field.center());
    step(&mut h, 2);
    h.event(egui::Event::PointerButton { pos: field.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    step(&mut h, 1);
    h.event(egui::Event::PointerButton { pos: field.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    step(&mut h, 3);
    assert!(h.ctx.text_edit_focused(), "the click did not put the cursor in the field");
    h.key_press_modifiers(cmd, egui::Key::Z);
    step(&mut h, 3);
    assert!(!h.state().is_example, "Ctrl+Z in a text field undid the whole CV");
    h.ctx.memory_mut(|m| m.stop_text_input());
    step(&mut h, 2);

    // Ctrl+P on a CV with things missing: the question, whose Enter shows the checklist (it
    // downloads nothing and loses nothing).
    h.state_mut().side = Side::Chat;
    h.key_press_modifiers(cmd, egui::Key::P);
    step(&mut h, 3);
    assert!(h.state().confirm.is_some(), "Ctrl+P did not ask before downloading");
    // While it is open, the main window's keys do nothing.
    h.key_press_modifiers(cmd, egui::Key::Z);
    step(&mut h, 3);
    assert!(!h.state().is_example, "Ctrl+Z went through the dialog");
    h.key_press(egui::Key::Enter);
    step(&mut h, 3);
    assert!(h.state().confirm.is_none() && h.state().side == Side::Checks, "Enter did not pick «Vis sjekkliste»");

    // Esc closes a dialog, and Enter presses its «Lukk».
    h.state_mut().about_open = true;
    step(&mut h, 3);
    h.key_press(egui::Key::Escape);
    step(&mut h, 3);
    assert!(!h.state().about_open, "Esc did not close «Om CVBuilder»");
    h.state_mut().about_open = true;
    step(&mut h, 3);
    h.key_press(egui::Key::Enter);
    step(&mut h, 3);
    assert!(!h.state().about_open, "Enter did not press «Lukk»");

    // The language editor with a changed translation stays open on Esc and on a click beside it;
    // «Avbryt» closes it.
    h.state_mut().open_editor("de");
    step(&mut h, 3);
    assert!(h.state().langs.editor.is_some());
    h.key_press(egui::Key::Escape);
    step(&mut h, 3);
    assert!(h.state().langs.editor.is_none(), "Esc did not close the unchanged editor");
    h.state_mut().open_editor("de");
    step(&mut h, 3);
    // The first field is the search; the second is a translation.
    let mut fields: Vec<Rect> = snapshot(&h).into_iter().filter(|c| c.role == Role::TextInput).map(|c| c.rect).collect();
    fields.sort_by(|a, b| a.top().total_cmp(&b.top()));
    let row = fields.get(1).copied().expect("a translation field");
    h.event(egui::Event::PointerButton { pos: row.center(), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    step(&mut h, 1);
    h.event(egui::Event::PointerButton { pos: row.center(), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    step(&mut h, 2);
    h.event(egui::Event::Text("x".into()));
    step(&mut h, 2);
    h.ctx.memory_mut(|m| m.stop_text_input());
    step(&mut h, 2);
    h.key_press(egui::Key::Escape);
    step(&mut h, 3);
    assert!(h.state().langs.editor.is_some(), "Esc closed the editor and lost the edit");
    let beside = Pos2::new(5.0, 450.0);
    h.hover_at(beside);
    step(&mut h, 2);
    h.event(egui::Event::PointerButton { pos: beside, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    step(&mut h, 1);
    h.event(egui::Event::PointerButton { pos: beside, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    step(&mut h, 3);
    assert!(h.state().langs.editor.is_some(), "a click beside the editor closed it and lost the edit");
    click(&mut h, "Avbryt");
    assert!(h.state().langs.editor.is_none(), "«Avbryt» did not close the editor");
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

/// The requirement and spell-check scenes in a tall window, so the whole side panel is in the picture:
/// `CVBUILDER_SHOTS=<dir> cargo test --release -p cvbuilder-app requirement_shots -- --ignored`
#[test]
#[ignore = "writes pictures; run on purpose"]
fn requirement_shots() {
    let Ok(dir) = std::env::var("CVBUILDER_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("shots folder");
    let lang = std::env::var("CVBUILDER_QA_LANGS").unwrap_or_else(|_| "no".into());
    private_data_folder(&lang);
    for scene in SCENES.iter().filter(|s| s.name.starts_with("Krav i") || s.name == "Stavekontroll" || s.name == "Sjekkliste") {
        let mut h = harness(Vec2::new(1440.0, 2600.0), scene.setup);
        let image = h.render().expect("render");
        image.save(dir.join(format!("{lang}-{}.png", scene.name.replace(' ', "_")))).expect("save");
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
        let image = h.render().expect("render");
        image.save(dir.join(format!("{lang}-{name}.png"))).expect("save");
    }
}

/// Before a one-click AI button sends the CV to a company, «Før AI-en starter» names it and the
/// cost; «Avbryt» sends nothing, «Send» is remembered for that kind of work, and a model on this
/// computer never asks (to-do 030).
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn ai_asks_before_the_cv_leaves() {
    use cv_ai::cost::Work;
    private_data_folder("no");
    i18n::set(i18n::Lang::No);
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| {
        with_provider(a);
        a.ask_ai(Work::Tailor, "Tilpass CV-en til stillingsannonsen.");
    });
    assert!(h.state().ai_pending.is_some(), "nothing asked before sending");
    assert!(!h.state().chat.running, "the request went out before the person said yes");
    // (snapshot() shortens long labels, so read the whole text here.)
    let texts: Vec<String> = h.query_all(by().include_labels().predicate(|_| true)).map(|n| { let ak = n.accesskit_node(); format!("{} {}", ak.label().unwrap_or_default(), ak.value().unwrap_or_default()) }).collect();
    let named = texts.iter().any(|l| l.contains("OpenAI") && l.contains("US$"));
    assert!(named, "the window does not name OpenAI and the cost:\n{}", texts.iter().filter(|l| l.contains("OpenAI") || l.contains("AI")).cloned().collect::<Vec<_>>().join("\n"));
    assert!(has_label(&h, "Send til OpenAI"), "no «Send til OpenAI»");
    click(&mut h, "Avbryt");
    assert!(h.state().ai_pending.is_none() && !h.state().chat.running && h.state().chat.items.is_empty(), "«Avbryt» did not stop the request");
    // Seeing the notice alone remembers nothing past the next request.
    h.state_mut().ai_seen(Work::Tailor);
    assert!(h.state_mut().must_ask(Work::Tailor).is_none(), "the notice was just shown");
    assert!(h.state_mut().must_ask(Work::Tailor).is_some(), "a yes was kept although nothing was sent");
    // Once the text has gone, the same kind of work goes straight on; another kind asks again.
    h.state_mut().ai_sent(Work::Tailor);
    assert!(h.state_mut().must_ask(Work::Tailor).is_none());
    assert!(h.state_mut().must_ask(Work::Translate).is_some());
    // A model on this computer: nothing leaves, nothing to ask.
    let mut h = harness(Vec2::new(1440.0, 900.0), with_local);
    assert!(h.state_mut().must_ask(Work::Tailor).is_none());
    // A server elsewhere: a yes counts for that address only, and a yes kept under the bare
    // service name (as before to-do 163) counts as not asked.
    h.state_mut().settings.local_url = "http://192.168.1.20:11434/v1".into();
    let name = h.state().chat_backend().expect("backend").name();
    h.state_mut().ai_ok.insert((Work::Tailor, name));
    assert!(h.state_mut().must_ask(Work::Tailor).is_some(), "an old yes by service name was taken as a yes");
    h.state_mut().ai_sent(Work::Tailor);
    assert!(h.state_mut().must_ask(Work::Tailor).is_none());
    h.state_mut().settings.local_url = "http://192.168.1.99:11434/v1".into();
    assert!(h.state_mut().must_ask(Work::Tailor).is_some(), "a new address did not ask again");
    // A user name in the address does not make another host local.
    h.state_mut().settings.local_url = "http://localhost:x@evil.example/v1".into();
    assert!(h.state_mut().must_ask(Work::Tailor).is_some(), "userinfo hid the real host");
    // The translate window says where the CV goes before «Oversett med AI».
    let h = harness(Vec2::new(1440.0, 900.0), |a| {
        with_provider(a);
        a.open_translate(None);
    });
    assert!(snapshot(&h).iter().any(|c| c.label.contains("OpenAI")), "the translate window does not say where the CV goes");
    // The leftovers after an import without AI: copying is the main button, the AI one says where.
    let h = harness(Vec2::new(1440.0, 900.0), |a| {
        with_provider(a);
        a.import.rest = Some("ERFARING".into());
    });
    assert!(has_label(&h, "La AI-en legge det inn (sendes til OpenAI)"));
}

/// Hiding (041): the eye takes an item out of this CV and «Vis alt igjen» brings it back; a
/// tailored copy is saved next to the full CV, which stays as it was, and knows where it is.
#[test]
#[ignore = "slow: part of the ui_qa run"]
fn hiding_items_and_saving_a_tailored_copy() {
    private_data_folder("no");
    let mut h = harness(Vec2::new(1440.0, 900.0), |a| a.folded.clear());
    let eye = "Ta ut av denne CV-en. Det blir stående i skjemaet og i filen.";
    let first = h.query_all(by().include_labels().predicate(move |n| n.label().as_deref() == Some(eye))).next().expect("an eye button");
    first.scroll_to_me();
    step(&mut h, 10);
    let at = h.query_all(by().include_labels().predicate(move |n| n.label().as_deref() == Some(eye))).next().map(|n| n.rect().center()).expect("eye");
    h.hover_at(at);
    step(&mut h, 2);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    step(&mut h, 1);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    step(&mut h, 4);
    assert_eq!(h.state().project.hidden_count(), 1, "the eye hid one item");
    assert!(has_label(&h, "Vis alt igjen"), "the form says something is hidden");
    assert!(has_label(&h, "1 skjult"), "the status bar counts it");
    // The line is at the top of the form, scrolled away while the eye was clicked.
    h.query_all(by().include_labels().predicate(|n| n.label().as_deref() == Some("Vis alt igjen"))).next().expect("Vis alt igjen").scroll_to_me();
    step(&mut h, 10);
    click(&mut h, "Vis alt igjen");
    assert_eq!(h.state().project.hidden_count(), 0, "«Vis alt igjen» showed it again");
    assert!(h.state().history.can_undo(), "and can be undone");

    // A tailored copy: the full CV's file is left as it is.
    let dir = std::env::temp_dir().join(format!("cvbuilder-copy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("folder");
    let (full, copy) = (dir.join("Ingrid_CV_utkast.json"), dir.join("Ingrid_CV_Acme_utkast.json"));
    let app = h.state_mut();
    app.is_example = false;
    app.save_to(&full, false);
    let job = app.project.experience[1].id.clone();
    app.project.set_hidden(&job, true);
    app.save_copy_to(&copy);
    assert_eq!(app.file.as_deref(), Some(copy.as_path()), "the copy is the open CV now");
    let saved = cv_core::Project::from_json(&std::fs::read_to_string(&copy).expect("copy")).expect("reads");
    assert_eq!(saved.options.copy_of, "Ingrid_CV_utkast.json");
    assert!(saved.is_hidden(&job));
    let untouched = cv_core::Project::from_json(&std::fs::read_to_string(&full).expect("full")).expect("reads");
    assert!(untouched.options.hidden.is_empty() && untouched.options.copy_of.is_empty(), "the full CV keeps everything");
    // Saving the copy over the full CV is refused.
    let before = std::fs::read_to_string(&full).expect("full");
    app.file = Some(full.clone());
    app.save_copy_to(&full);
    assert_eq!(std::fs::read_to_string(&full).expect("full"), before);
    let _ = std::fs::remove_dir_all(&dir);
}
