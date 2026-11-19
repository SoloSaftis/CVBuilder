//! Renders the first page of an Arabic and a Hindi CV to PNG (and writes the PDF and Word files),
//! for checking right-to-left layout and shaped scripts by eye: `script_shots <out-dir> [design-id …]`.
use cv_core::{Project, document, headings};

fn sample(language: &str, name: &str, headline: &str, summary: &str, bullets: [&str; 2], city: &str) -> Project {
    let mut p = Project::example_en();
    p.options.headings = headings::built_in(language);
    p.candidate.name = name.into();
    p.candidate.headline = headline.into();
    p.candidate.summary = summary.into();
    p.candidate.city = city.into();
    if let Some(job) = p.experience.first_mut() {
        job.bullets = bullets.iter().map(|b| b.to_string()).collect();
    }
    p
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| ".".into());
    let designs: Vec<String> = args.collect();
    let cvs = [
        (
            "ar",
            sample(
                "العربية",
                "سارة الأحمد",
                "مهندسة برمجيات",
                "مهندسة برمجيات بخبرة ثماني سنوات في تطوير الأنظمة. أقود فرقًا صغيرة وأسلّم منتجات موثوقة.",
                ["قدت فريقًا من خمسة مطورين لإطلاق تطبيق جديد.", "خفضت وقت التحميل بنسبة 40٪."],
                "الرياض",
            ),
        ),
        (
            "hi",
            sample(
                "हिन्दी",
                "प्रिया शर्मा",
                "सॉफ़्टवेयर इंजीनियर",
                "आठ वर्षों के अनुभव वाली सॉफ़्टवेयर इंजीनियर। मैं छोटी टीमों का नेतृत्व करती हूँ।",
                ["पाँच डेवलपर्स की टीम का नेतृत्व किया।", "लोड समय 40% कम किया।"],
                "दिल्ली",
            ),
        ),
    ];
    for (code, mut project) in cvs {
        let ids = if designs.is_empty() { vec![project.options.layout.clone()] } else { designs.clone() };
        for id in ids {
            project.options.layout = id.clone();
            let doc = document::build(&project);
            std::fs::write(format!("{out}/{code}-{id}.pdf"), document::to_pdf(&doc)?)?;
            std::fs::write(format!("{out}/{code}-{id}.docx"), document::to_docx(&doc)?)?;
            let layout = wordcraft_layout::layout(&doc, &mut wordcraft_layout::LayoutCache::new(), &Default::default());
            if let Some(page) = layout.pages.first() {
                let r = wordcraft_render::render_page(&doc, page, 1.5, &wordcraft_render::RenderOptions::default());
                let img = image::RgbaImage::from_raw(r.width, r.height, r.pixels).ok_or("bad page image")?;
                img.save(format!("{out}/{code}-{id}.png"))?;
            }
            println!("{code} {id}");
        }
    }
    Ok(())
}
