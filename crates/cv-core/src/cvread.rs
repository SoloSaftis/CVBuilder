//! Reading an existing CV without AI: the text of a PDF, Word, OpenDocument, RTF or text file
//! becomes a filled-in project. It takes what it can recognise with confidence – contact
//! details, the profile, jobs and education with their dates, skills, languages, courses,
//! interests and sections such as projects – and leaves the rest as text for the person (or the
//! AI) to place. Nothing is invented: every value is a piece of the CV's own text.
use std::sync::LazyLock;

use regex::Regex;

use crate::checks;
use crate::import::tidy;
use crate::model::{Certification, ContactField, CustomItem, CustomSection, Education, Interest, Job, Project, Skill, SpokenLanguage};

/// What could be read from an existing CV without AI, and the text that was not used.
#[derive(Debug, Clone, Default)]
pub struct CvRead {
    pub project: Project,
    /// The parts that could not be placed, for the person (or the AI) to sort.
    pub rest: String,
    /// What was found, as short Norwegian words («navn», «jobber» …) for the status line.
    pub found: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Top,
    Profile,
    Skills,
    Languages,
    Experience,
    Education,
    Certifications,
    Interests,
    /// A section of its own, such as projects or voluntary work: (Norwegian title, English title).
    Custom(&'static str, &'static str),
    /// Read but not used: references, personal details.
    Skip,
}

/// The headings CVs use, in Norwegian and English.
const HEADINGS: &[(&[&str], Part)] = &[
    (&["profil", "profile", "sammendrag", "summary", "om meg", "about me", "about", "personlig profil", "professional summary", "career summary", "objective", "career objective"], Part::Profile),
    (
        &[
            "kjernekompetanse",
            "kompetanse",
            "ferdigheter",
            "skills",
            "core skills",
            "key skills",
            "technical skills",
            "nøkkelkompetanse",
            "competencies",
            "core competencies",
            "expertise",
            "verktøy",
            "tools",
            "it-kompetanse",
        ],
        Part::Skills,
    ),
    (&["språk", "languages", "language skills", "språkkunnskaper", "language"], Part::Languages),
    (
        &[
            "arbeidserfaring",
            "erfaring",
            "experience",
            "work experience",
            "professional experience",
            "yrkeserfaring",
            "employment",
            "employment history",
            "work history",
            "relevant erfaring",
            "career history",
        ],
        Part::Experience,
    ),
    (&["utdanning", "education", "utdannelse", "academic background", "qualifications", "skolegang"], Part::Education),
    (
        &[
            "kurs",
            "sertifiseringer",
            "sertifikater",
            "kurs og sertifiseringer",
            "kurs og sertifikater",
            "kurs og kompetansebevis",
            "sertifiseringer og kurs",
            "certifications",
            "courses",
            "certificates",
            "certifications and courses",
            "courses and certifications",
            "courses & certifications",
            "certifications & courses",
            "licenses",
            "licences",
            "licenses and certifications",
            "training",
        ],
        Part::Certifications,
    ),
    (&["interesser", "interests", "hobbyer", "hobbies", "fritid", "fritidsinteresser"], Part::Interests),
    (&["prosjekter", "projects", "utvalgte prosjekter", "selected projects"], Part::Custom("Prosjekter", "Projects")),
    (&["frivillig arbeid", "frivillig", "volunteering", "volunteer", "volunteer experience", "community"], Part::Custom("Frivillig arbeid", "Volunteering")),
    (&["verv", "tillitsverv", "verv og styreverv", "positions of trust", "board positions"], Part::Custom("Verv og styreverv", "Positions of trust")),
    (&["publikasjoner", "publications"], Part::Custom("Publikasjoner", "Publications")),
    (&["priser", "utmerkelser", "priser og utmerkelser", "awards", "honours", "honors", "achievements"], Part::Custom("Priser og utmerkelser", "Awards")),
    (&["referanser", "references", "personalia", "kontakt", "contact", "kontaktinformasjon", "contact information", "personal details"], Part::Skip),
];

/// «S U M M A RY», «E X P E R I E N C E»: headings spaced out letter by letter.
fn unspace(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    let single = words.iter().filter(|w| w.chars().count() == 1 && w.chars().all(char::is_alphabetic)).count();
    if words.len() >= 4 && single * 10 >= words.len() * 6 {
        // Words are separated by two or more spaces; letters by one.
        line.split("  ").map(|w| w.split_whitespace().collect::<String>()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
    } else {
        line.to_string()
    }
}

/// Headings of CVs in the other languages the app knows, by the language's own name (as in
/// `headings`), so a German CV is read as German and gets German headings.
const FOREIGN: &[(&str, Part, &[&str])] = &[
    // Deutsch
    ("Deutsch", Part::Profile, &["profil", "kurzprofil", "über mich", "zusammenfassung", "persönliches profil", "berufliches profil"]),
    ("Deutsch", Part::Skills, &["kernkompetenzen", "kompetenzen", "kenntnisse", "fähigkeiten", "fachkenntnisse", "it-kenntnisse", "edv-kenntnisse", "fähigkeiten und kenntnisse", "qualifikationen"]),
    ("Deutsch", Part::Languages, &["sprachen", "sprachkenntnisse", "fremdsprachen"]),
    ("Deutsch", Part::Experience, &["berufserfahrung", "beruflicher werdegang", "werdegang", "erfahrung", "praktische erfahrung", "berufspraxis"]),
    ("Deutsch", Part::Education, &["ausbildung", "bildungsweg", "schulbildung", "studium", "akademischer werdegang", "aus- und weiterbildung"]),
    ("Deutsch", Part::Certifications, &["weiterbildungen und zertifikate", "weiterbildung", "weiterbildungen", "zertifikate", "zertifizierungen", "fortbildungen", "kurse"]),
    ("Deutsch", Part::Interests, &["interessen", "hobbys", "hobbies", "freizeit", "freizeitinteressen"]),
    ("Deutsch", Part::Custom("Prosjekter", "Projects"), &["projekte", "projekterfahrung"]),
    ("Deutsch", Part::Custom("Frivillig arbeid", "Volunteering"), &["ehrenamt", "ehrenamtliches engagement", "engagement"]),
    ("Deutsch", Part::Skip, &["persönliche daten", "kontaktdaten", "referenzen"]),
    // Français
    ("Français", Part::Profile, &["profil", "à propos", "résumé", "profil professionnel", "objectif"]),
    ("Français", Part::Skills, &["compétences", "compétences clés", "compétences techniques", "savoir-faire", "outils"]),
    ("Français", Part::Languages, &["langues", "langues étrangères", "compétences linguistiques"]),
    ("Français", Part::Experience, &["expérience professionnelle", "expériences professionnelles", "expérience", "expériences", "parcours professionnel"]),
    ("Français", Part::Education, &["formation", "formations", "éducation", "parcours académique", "études", "diplômes"]),
    ("Français", Part::Certifications, &["formations et certifications", "certifications", "certificats"]),
    ("Français", Part::Interests, &["centres d'intérêt", "centres d’intérêt", "loisirs", "intérêts", "activités"]),
    ("Français", Part::Custom("Prosjekter", "Projects"), &["projets"]),
    ("Français", Part::Custom("Frivillig arbeid", "Volunteering"), &["bénévolat", "engagement associatif"]),
    ("Français", Part::Skip, &["coordonnées", "références", "informations personnelles"]),
    // Español
    ("Español", Part::Profile, &["perfil", "perfil profesional", "sobre mí", "resumen", "extracto", "objetivo"]),
    ("Español", Part::Skills, &["competencias clave", "competencias", "habilidades", "aptitudes", "conocimientos", "herramientas"]),
    ("Español", Part::Languages, &["idiomas", "lenguas"]),
    ("Español", Part::Experience, &["experiencia profesional", "experiencia laboral", "experiencia", "trayectoria profesional"]),
    ("Español", Part::Education, &["formación", "formación académica", "educación", "estudios"]),
    ("Español", Part::Certifications, &["cursos y certificaciones", "cursos", "certificaciones", "formación complementaria"]),
    ("Español", Part::Interests, &["intereses", "aficiones", "pasatiempos"]),
    ("Español", Part::Custom("Prosjekter", "Projects"), &["proyectos"]),
    ("Español", Part::Custom("Frivillig arbeid", "Volunteering"), &["voluntariado"]),
    ("Español", Part::Skip, &["datos personales", "datos de contacto", "referencias"]),
    // Português
    ("Português", Part::Profile, &["perfil", "perfil profissional", "sobre mim", "resumo", "objetivo"]),
    ("Português", Part::Skills, &["competências principais", "competências", "habilidades", "conhecimentos", "ferramentas"]),
    ("Português", Part::Languages, &["idiomas", "línguas"]),
    ("Português", Part::Experience, &["experiência profissional", "experiência", "percurso profissional"]),
    ("Português", Part::Education, &["formação", "formação académica", "formação acadêmica", "educação", "escolaridade"]),
    ("Português", Part::Certifications, &["cursos e certificações", "cursos", "certificações"]),
    ("Português", Part::Interests, &["interesses", "passatempos", "hobbies"]),
    ("Português", Part::Custom("Prosjekter", "Projects"), &["projetos", "projectos"]),
    ("Português", Part::Custom("Frivillig arbeid", "Volunteering"), &["voluntariado"]),
    ("Português", Part::Skip, &["dados pessoais", "contactos", "referências"]),
    // Italiano
    ("Italiano", Part::Profile, &["profilo", "profilo professionale", "chi sono", "sommario", "obiettivo"]),
    ("Italiano", Part::Skills, &["competenze chiave", "competenze", "capacità", "conoscenze", "competenze tecniche", "strumenti"]),
    ("Italiano", Part::Languages, &["lingue", "conoscenze linguistiche", "lingue straniere"]),
    ("Italiano", Part::Experience, &["esperienza professionale", "esperienze professionali", "esperienza lavorativa", "esperienze lavorative", "esperienza"]),
    ("Italiano", Part::Education, &["istruzione", "formazione", "istruzione e formazione", "studi"]),
    ("Italiano", Part::Certifications, &["corsi e certificazioni", "corsi", "certificazioni"]),
    ("Italiano", Part::Interests, &["interessi", "hobby", "passioni"]),
    ("Italiano", Part::Custom("Prosjekter", "Projects"), &["progetti"]),
    ("Italiano", Part::Custom("Frivillig arbeid", "Volunteering"), &["volontariato"]),
    ("Italiano", Part::Skip, &["dati personali", "contatti", "referenze"]),
    // Nederlands
    ("Nederlands", Part::Profile, &["profiel", "over mij", "samenvatting", "persoonlijk profiel"]),
    ("Nederlands", Part::Skills, &["kerncompetenties", "competenties", "vaardigheden", "kennis", "tools"]),
    ("Nederlands", Part::Languages, &["talen", "talenkennis"]),
    ("Nederlands", Part::Experience, &["werkervaring", "ervaring", "loopbaan", "relevante ervaring"]),
    ("Nederlands", Part::Education, &["opleiding", "opleidingen", "onderwijs"]),
    ("Nederlands", Part::Certifications, &["cursussen en certificaten", "cursussen", "certificaten", "trainingen"]),
    ("Nederlands", Part::Interests, &["interesses", "hobby's", "hobbies"]),
    ("Nederlands", Part::Custom("Prosjekter", "Projects"), &["projecten"]),
    ("Nederlands", Part::Custom("Frivillig arbeid", "Volunteering"), &["vrijwilligerswerk"]),
    ("Nederlands", Part::Skip, &["personalia", "persoonlijke gegevens", "referenties"]),
    // Svenska
    ("Svenska", Part::Profile, &["profil", "om mig", "sammanfattning"]),
    ("Svenska", Part::Skills, &["kärnkompetens", "kompetenser", "kompetens", "färdigheter", "kunskaper", "verktyg"]),
    ("Svenska", Part::Languages, &["språk", "språkkunskaper"]),
    ("Svenska", Part::Experience, &["arbetslivserfarenhet", "erfarenhet", "yrkeserfarenhet", "anställningar"]),
    ("Svenska", Part::Education, &["utbildning", "utbildningar"]),
    ("Svenska", Part::Certifications, &["kurser och certifieringar", "kurser", "certifieringar", "certifikat"]),
    ("Svenska", Part::Interests, &["intressen", "fritidsintressen"]),
    ("Svenska", Part::Custom("Prosjekter", "Projects"), &["projekt"]),
    ("Svenska", Part::Custom("Frivillig arbeid", "Volunteering"), &["ideellt arbete", "föreningsliv"]),
    ("Svenska", Part::Skip, &["personuppgifter", "kontaktuppgifter", "referenser"]),
    // Dansk
    ("Dansk", Part::Profile, &["profil", "om mig", "resumé"]),
    ("Dansk", Part::Skills, &["kernekompetencer", "kompetencer", "færdigheder", "kvalifikationer", "værktøjer"]),
    ("Dansk", Part::Languages, &["sprog", "sprogkundskaber"]),
    ("Dansk", Part::Experience, &["erhvervserfaring", "erfaring", "arbejdserfaring", "beskæftigelse"]),
    ("Dansk", Part::Education, &["uddannelse", "uddannelser"]),
    ("Dansk", Part::Certifications, &["kurser og certificeringer", "kurser", "certificeringer", "efteruddannelse"]),
    ("Dansk", Part::Interests, &["interesser", "fritidsinteresser", "fritid"]),
    ("Dansk", Part::Custom("Prosjekter", "Projects"), &["projekter"]),
    ("Dansk", Part::Custom("Frivillig arbeid", "Volunteering"), &["frivilligt arbejde"]),
    ("Dansk", Part::Skip, &["personlige oplysninger", "kontaktoplysninger", "referencer"]),
    // Suomi
    ("Suomi", Part::Profile, &["profiili", "tietoa minusta", "yhteenveto"]),
    ("Suomi", Part::Skills, &["ydinosaaminen", "osaaminen", "taidot", "erityisosaaminen", "työkalut"]),
    ("Suomi", Part::Languages, &["kielitaito", "kielet"]),
    ("Suomi", Part::Experience, &["työkokemus", "työhistoria", "kokemus"]),
    ("Suomi", Part::Education, &["koulutus", "opinnot"]),
    ("Suomi", Part::Certifications, &["kurssit ja sertifikaatit", "kurssit", "sertifikaatit", "lisäkoulutus"]),
    ("Suomi", Part::Interests, &["kiinnostuksen kohteet", "harrastukset"]),
    ("Suomi", Part::Custom("Prosjekter", "Projects"), &["projektit"]),
    ("Suomi", Part::Custom("Frivillig arbeid", "Volunteering"), &["vapaaehtoistyö", "luottamustoimet"]),
    ("Suomi", Part::Skip, &["henkilötiedot", "yhteystiedot", "suosittelijat"]),
    // Polski
    ("Polski", Part::Profile, &["profil", "o mnie", "podsumowanie", "profil zawodowy"]),
    ("Polski", Part::Skills, &["kluczowe kompetencje", "kompetencje", "umiejętności", "znajomość narzędzi"]),
    ("Polski", Part::Languages, &["języki", "języki obce", "znajomość języków"]),
    ("Polski", Part::Experience, &["doświadczenie zawodowe", "doświadczenie", "przebieg kariery"]),
    ("Polski", Part::Education, &["wykształcenie", "edukacja"]),
    ("Polski", Part::Certifications, &["kursy i certyfikaty", "kursy", "certyfikaty", "szkolenia"]),
    ("Polski", Part::Interests, &["zainteresowania", "hobby"]),
    ("Polski", Part::Custom("Prosjekter", "Projects"), &["projekty"]),
    ("Polski", Part::Custom("Frivillig arbeid", "Volunteering"), &["wolontariat"]),
    ("Polski", Part::Skip, &["dane osobowe", "dane kontaktowe", "referencje"]),
    // Čeština
    ("Čeština", Part::Profile, &["profil", "o mně", "shrnutí"]),
    ("Čeština", Part::Skills, &["klíčové kompetence", "dovednosti", "znalosti", "kompetence"]),
    ("Čeština", Part::Languages, &["jazyky", "jazykové znalosti"]),
    ("Čeština", Part::Experience, &["pracovní zkušenosti", "praxe", "zkušenosti"]),
    ("Čeština", Part::Education, &["vzdělání", "studium"]),
    ("Čeština", Part::Certifications, &["kurzy a certifikáty", "kurzy", "certifikáty", "školení"]),
    ("Čeština", Part::Interests, &["zájmy", "koníčky"]),
    ("Čeština", Part::Custom("Prosjekter", "Projects"), &["projekty"]),
    ("Čeština", Part::Skip, &["osobní údaje", "kontakt", "reference"]),
    // Română
    ("Română", Part::Profile, &["profil", "despre mine", "rezumat", "obiectiv"]),
    ("Română", Part::Skills, &["competențe cheie", "competențe", "abilități", "aptitudini", "cunoștințe"]),
    ("Română", Part::Languages, &["limbi străine", "limbi", "competențe lingvistice"]),
    ("Română", Part::Experience, &["experiență profesională", "experiență", "experiența profesională"]),
    ("Română", Part::Education, &["educație", "studii", "educație și formare"]),
    ("Română", Part::Certifications, &["cursuri și certificări", "cursuri", "certificări"]),
    ("Română", Part::Interests, &["interese", "hobby-uri"]),
    ("Română", Part::Custom("Prosjekter", "Projects"), &["proiecte"]),
    ("Română", Part::Custom("Frivillig arbeid", "Volunteering"), &["voluntariat"]),
    ("Română", Part::Skip, &["date personale", "informații personale", "referințe"]),
    // Magyar
    ("Magyar", Part::Profile, &["profil", "bemutatkozás", "összefoglaló", "rólam"]),
    ("Magyar", Part::Skills, &["kulcskompetenciák", "kompetenciák", "készségek", "szakmai ismeretek", "ismeretek"]),
    ("Magyar", Part::Languages, &["nyelvtudás", "nyelvismeret", "nyelvek"]),
    ("Magyar", Part::Experience, &["szakmai tapasztalat", "munkatapasztalat", "tapasztalat"]),
    ("Magyar", Part::Education, &["tanulmányok", "végzettség", "iskolai végzettség", "oktatás"]),
    ("Magyar", Part::Certifications, &["tanfolyamok és tanúsítványok", "tanfolyamok", "tanúsítványok", "képzések"]),
    ("Magyar", Part::Interests, &["érdeklődési kör", "hobbi", "hobbik"]),
    ("Magyar", Part::Custom("Prosjekter", "Projects"), &["projektek"]),
    ("Magyar", Part::Skip, &["személyes adatok", "elérhetőség", "referenciák"]),
    // Türkçe
    ("Türkçe", Part::Profile, &["profil", "hakkımda", "özet", "kariyer hedefi"]),
    ("Türkçe", Part::Skills, &["temel yetkinlikler", "yetkinlikler", "yetenekler", "beceriler", "bilgisayar becerileri"]),
    ("Türkçe", Part::Languages, &["diller", "yabancı diller", "yabancı dil"]),
    ("Türkçe", Part::Experience, &["iş deneyimi", "deneyim", "iş tecrübesi", "tecrübe", "profesyonel deneyim"]),
    ("Türkçe", Part::Education, &["eğitim", "eğitim bilgileri", "öğrenim"]),
    ("Türkçe", Part::Certifications, &["kurslar ve sertifikalar", "kurslar", "sertifikalar", "eğitimler ve sertifikalar"]),
    ("Türkçe", Part::Interests, &["ilgi alanları", "hobiler"]),
    ("Türkçe", Part::Custom("Prosjekter", "Projects"), &["projeler"]),
    ("Türkçe", Part::Custom("Frivillig arbeid", "Volunteering"), &["gönüllülük", "gönüllü çalışmalar"]),
    ("Türkçe", Part::Skip, &["kişisel bilgiler", "iletişim bilgileri", "referanslar"]),
    // Русский
    ("Русский", Part::Profile, &["профиль", "о себе", "резюме", "цель"]),
    ("Русский", Part::Skills, &["ключевые навыки", "навыки", "профессиональные навыки", "компетенции"]),
    ("Русский", Part::Languages, &["языки", "знание языков", "иностранные языки"]),
    ("Русский", Part::Experience, &["опыт работы", "опыт", "трудовая деятельность"]),
    ("Русский", Part::Education, &["образование"]),
    ("Русский", Part::Certifications, &["курсы и сертификаты", "курсы", "сертификаты", "повышение квалификации"]),
    ("Русский", Part::Interests, &["интересы", "хобби", "увлечения"]),
    ("Русский", Part::Custom("Prosjekter", "Projects"), &["проекты"]),
    ("Русский", Part::Skip, &["личные данные", "контакты", "рекомендации"]),
    // Українська
    ("Українська", Part::Profile, &["профіль", "про себе", "резюме", "мета"]),
    ("Українська", Part::Skills, &["ключові компетенції", "навички", "ключові навички", "компетенції"]),
    ("Українська", Part::Languages, &["мови", "знання мов", "іноземні мови"]),
    ("Українська", Part::Experience, &["досвід роботи", "досвід"]),
    ("Українська", Part::Education, &["освіта"]),
    ("Українська", Part::Certifications, &["курси та сертифікати", "курси", "сертифікати"]),
    ("Українська", Part::Interests, &["інтереси", "хобі", "захоплення"]),
    ("Українська", Part::Custom("Prosjekter", "Projects"), &["проєкти", "проекти"]),
    ("Українська", Part::Skip, &["особисті дані", "контакти", "рекомендації"]),
    // Ελληνικά
    ("Ελληνικά", Part::Profile, &["προφίλ", "σχετικά με εμένα", "περίληψη"]),
    ("Ελληνικά", Part::Skills, &["βασικές δεξιότητες", "δεξιότητες", "ικανότητες", "γνώσεις"]),
    ("Ελληνικά", Part::Languages, &["γλώσσες", "ξένες γλώσσες"]),
    ("Ελληνικά", Part::Experience, &["επαγγελματική εμπειρία", "εμπειρία", "εργασιακή εμπειρία"]),
    ("Ελληνικά", Part::Education, &["εκπαίδευση", "σπουδές"]),
    ("Ελληνικά", Part::Certifications, &["σεμινάρια και πιστοποιήσεις", "σεμινάρια", "πιστοποιήσεις"]),
    ("Ελληνικά", Part::Interests, &["ενδιαφέροντα", "χόμπι"]),
    ("Ελληνικά", Part::Skip, &["προσωπικά στοιχεία", "στοιχεία επικοινωνίας"]),
    // Bahasa Indonesia
    ("Bahasa Indonesia", Part::Profile, &["profil", "tentang saya", "ringkasan"]),
    ("Bahasa Indonesia", Part::Skills, &["keahlian utama", "keahlian", "keterampilan", "kompetensi"]),
    ("Bahasa Indonesia", Part::Languages, &["bahasa", "kemampuan bahasa"]),
    ("Bahasa Indonesia", Part::Experience, &["pengalaman kerja", "pengalaman"]),
    ("Bahasa Indonesia", Part::Education, &["pendidikan", "riwayat pendidikan"]),
    ("Bahasa Indonesia", Part::Certifications, &["kursus dan sertifikasi", "sertifikasi", "pelatihan", "kursus"]),
    ("Bahasa Indonesia", Part::Interests, &["minat", "hobi"]),
    ("Bahasa Indonesia", Part::Custom("Prosjekter", "Projects"), &["proyek"]),
    ("Bahasa Indonesia", Part::Skip, &["data pribadi", "kontak", "referensi"]),
    // Tiếng Việt
    ("Tiếng Việt", Part::Profile, &["giới thiệu", "giới thiệu bản thân", "mục tiêu nghề nghiệp", "tóm tắt"]),
    ("Tiếng Việt", Part::Skills, &["kỹ năng chính", "kỹ năng"]),
    ("Tiếng Việt", Part::Languages, &["ngôn ngữ", "ngoại ngữ"]),
    ("Tiếng Việt", Part::Experience, &["kinh nghiệm làm việc", "kinh nghiệm"]),
    ("Tiếng Việt", Part::Education, &["học vấn", "trình độ học vấn"]),
    ("Tiếng Việt", Part::Certifications, &["khóa học và chứng chỉ", "chứng chỉ", "khóa học"]),
    ("Tiếng Việt", Part::Interests, &["sở thích"]),
    ("Tiếng Việt", Part::Custom("Prosjekter", "Projects"), &["dự án"]),
    ("Tiếng Việt", Part::Skip, &["thông tin cá nhân", "liên hệ", "người tham chiếu"]),
    // Íslenska, Lietuvių
    ("Íslenska", Part::Profile, &["prófíll", "um mig"]),
    ("Íslenska", Part::Skills, &["kjarnahæfni", "hæfni", "færni"]),
    ("Íslenska", Part::Languages, &["tungumál", "tungumálakunnátta"]),
    ("Íslenska", Part::Experience, &["starfsreynsla", "reynsla"]),
    ("Íslenska", Part::Education, &["menntun"]),
    ("Íslenska", Part::Certifications, &["námskeið og vottanir", "námskeið"]),
    ("Íslenska", Part::Interests, &["áhugamál"]),
    ("Lietuvių", Part::Profile, &["profilis", "apie mane"]),
    ("Lietuvių", Part::Skills, &["pagrindinės kompetencijos", "kompetencijos", "įgūdžiai"]),
    ("Lietuvių", Part::Languages, &["kalbos", "kalbų žinios"]),
    ("Lietuvių", Part::Experience, &["darbo patirtis", "patirtis"]),
    ("Lietuvių", Part::Education, &["išsilavinimas"]),
    ("Lietuvių", Part::Certifications, &["kursai ir sertifikatai", "kursai", "sertifikatai"]),
    ("Lietuvių", Part::Interests, &["pomėgiai", "interesai"]),
    // 日本語, 简体中文, 한국어
    ("日本語", Part::Profile, &["プロフィール", "自己pr", "職務要約"]),
    ("日本語", Part::Skills, &["スキル", "活かせる経験・知識・技術"]),
    ("日本語", Part::Languages, &["語学", "語学力"]),
    ("日本語", Part::Experience, &["職歴", "職務経歴"]),
    ("日本語", Part::Education, &["学歴"]),
    ("日本語", Part::Certifications, &["研修・資格", "資格", "免許・資格"]),
    ("日本語", Part::Interests, &["趣味・関心", "趣味"]),
    ("简体中文", Part::Profile, &["个人简介", "自我评价", "个人总结"]),
    ("简体中文", Part::Skills, &["核心技能", "专业技能", "技能"]),
    ("简体中文", Part::Languages, &["语言能力", "语言"]),
    ("简体中文", Part::Experience, &["工作经历", "工作经验"]),
    ("简体中文", Part::Education, &["教育背景", "教育经历"]),
    ("简体中文", Part::Certifications, &["培训与证书", "证书", "资格证书"]),
    ("简体中文", Part::Interests, &["兴趣爱好"]),
    ("简体中文", Part::Custom("Prosjekter", "Projects"), &["项目经历", "项目经验"]),
    ("한국어", Part::Profile, &["프로필", "자기소개", "요약"]),
    ("한국어", Part::Skills, &["핵심 역량", "역량", "기술", "보유 기술"]),
    ("한국어", Part::Languages, &["어학", "외국어"]),
    ("한국어", Part::Experience, &["경력", "경력 사항"]),
    ("한국어", Part::Education, &["학력", "학력 사항"]),
    ("한국어", Part::Certifications, &["교육 및 자격증", "자격증"]),
    ("한국어", Part::Interests, &["관심사", "취미"]),
];

/// A CV section heading such as «Arbeidserfaring» or «Skills» (for the job-ad reader, which skips them).
pub(crate) fn is_heading(text: &str) -> bool {
    heading_of(text).is_some()
}

fn heading_key(text: &str) -> Option<String> {
    let l = text.trim().trim_end_matches([':', '：']).trim().to_lowercase();
    (!l.is_empty() && l.chars().count() <= 40).then_some(l)
}

/// [`FOREIGN`] and the languages of assets/cv-languages.json together.
static ALL_FOREIGN: LazyLock<Vec<(&'static str, Part, Vec<&'static str>)>> = LazyLock::new(|| {
    let mut all: Vec<(&'static str, Part, Vec<&'static str>)> = FOREIGN.iter().map(|(l, p, w)| (*l, *p, w.to_vec())).collect();
    let more: &'static [crate::headings::CvLanguage] = &crate::headings::MORE;
    for lang in more {
        for (key, part) in [
            ("profile", Part::Profile),
            ("skills", Part::Skills),
            ("languages", Part::Languages),
            ("experience", Part::Experience),
            ("education", Part::Education),
            ("certifications", Part::Certifications),
            ("interests", Part::Interests),
            ("projects", Part::Custom("Prosjekter", "Projects")),
            ("volunteering", Part::Custom("Frivillig arbeid", "Volunteering")),
            ("skip", Part::Skip),
        ] {
            // The CV's own heading first: «heading_line» lets the first word share its line.
            let own = match part {
                Part::Profile => Some(0),
                Part::Skills => Some(1),
                Part::Experience => Some(2),
                Part::Education => Some(3),
                Part::Certifications => Some(4),
                Part::Languages => Some(5),
                Part::Interests => Some(6),
                _ => None,
            };
            let mut words: Vec<&'static str> = Vec::new();
            let mut add = |w: &'static str| {
                let w = w.trim();
                if !w.is_empty() && !words.iter().any(|x| x.to_lowercase() == w.to_lowercase()) {
                    words.push(w);
                }
            };
            if let Some(i) = own {
                add(leak_lower(&lang.headings[i]));
            }
            for w in lang.import.get(key).into_iter().flatten() {
                add(leak_lower(w));
            }
            if !words.is_empty() {
                all.push((lang.name.as_str(), part, words));
            }
        }
    }
    all
});

/// Lower case, kept for the rest of the run (the data is read once).
fn leak_lower(s: &str) -> &'static str {
    Box::leak(s.trim().to_lowercase().into_boxed_str())
}

fn heading_of(text: &str) -> Option<Part> {
    let l = heading_key(text)?;
    HEADINGS.iter().find(|(words, _)| words.contains(&l.as_str())).map(|(_, part)| *part).or_else(|| ALL_FOREIGN.iter().find(|(_, _, words)| words.contains(&l.as_str())).map(|(_, part, _)| *part))
}

/// The languages a heading is written in, when it is one of the others (not Norwegian or
/// English). Related languages share words («obrazovanje»), so every match counts.
fn heading_languages(text: &str) -> Vec<&'static str> {
    let Some(l) = heading_key(text) else { return Vec::new() };
    if HEADINGS.iter().any(|(words, _)| words.contains(&l.as_str())) {
        return Vec::new();
    }
    let mut out: Vec<&'static str> = Vec::new();
    for (lang, _, words) in ALL_FOREIGN.iter() {
        if words.contains(&l.as_str()) && !out.contains(lang) {
            out.push(lang);
        }
    }
    out
}

/// A heading alone on its line, or at the start of a line with the section's first words after
/// it («Experience Cedar Cloud Austin, TX»), as two-column PDFs often give. Returns the part and
/// what follows the heading.
fn heading_line(line: &str) -> Option<(Part, String)> {
    if let Some(part) = heading_of(line) {
        return Some((part, String::new()));
    }
    let words: Vec<&str> = line.split_whitespace().collect();
    let starts_upper = |w: &str| w.chars().next().is_some_and(char::is_uppercase);
    for n in (1..=3).rev() {
        if words.len() <= n || !starts_upper(words[0]) || !starts_upper(words[n]) {
            continue;
        }
        let head = words[..n].join(" ").to_lowercase();
        let head_key = head.trim_end_matches(':');
        // The main sections in the other languages may share their line too («Berufserfahrung Siemens AG …»).
        let foreign_main = ALL_FOREIGN.iter().any(|(_, part, words)| {
            matches!(part, Part::Profile | Part::Skills | Part::Languages | Part::Experience | Part::Education | Part::Certifications | Part::Interests) && words.first() == Some(&head_key)
        });
        if !PREFIX_HEADINGS.contains(&head_key) && !foreign_main {
            continue;
        }
        if let Some(part) = heading_of(&head) {
            // «Profile Senior Software Engineer with …» is a heading; «Experience with Excel» is not.
            return Some((part, words[n..].join(" ")));
        }
    }
    None
}

/// Headings that may share their line with the section's first words. Words such as «Community»
/// or «Training» also start names («Community Skills Network»), so they must stand alone.
const PREFIX_HEADINGS: &[&str] = &[
    "profil",
    "profile",
    "summary",
    "sammendrag",
    "experience",
    "work experience",
    "erfaring",
    "arbeidserfaring",
    "education",
    "utdanning",
    "skills",
    "kompetanse",
    "ferdigheter",
    "projects",
    "prosjekter",
    "languages",
    "språk",
    "certifications",
    "interests",
    "interesser",
];

// ---------- dates ----------

/// Month names CVs use, in the app's languages, with the month they stand for.
const MONTH_NAMES: &[(&str, u32)] = &[
    // Norwegian, English, Danish, Swedish
    ("jan", 1), ("januar", 1), ("january", 1), ("januari", 1), ("feb", 2), ("februar", 2), ("february", 2), ("februari", 2), ("mar", 3), ("mars", 3), ("march", 3), ("marts", 3),
    ("apr", 4), ("april", 4), ("mai", 5), ("may", 5), ("maj", 5), ("jun", 6), ("juni", 6), ("june", 6), ("jul", 7), ("juli", 7), ("july", 7), ("aug", 8), ("august", 8), ("augusti", 8),
    ("sep", 9), ("sept", 9), ("september", 9), ("okt", 10), ("oktober", 10), ("oct", 10), ("october", 10), ("nov", 11), ("november", 11), ("des", 12), ("desember", 12), ("dec", 12), ("december", 12),
    // German
    ("jänner", 1), ("märz", 3), ("mär", 3), ("mrz", 3), ("dez", 12), ("dezember", 12),
    // French
    ("janv", 1), ("janvier", 1), ("févr", 2), ("février", 2), ("fevrier", 2), ("avr", 4), ("avril", 4), ("juin", 6), ("juil", 7), ("juillet", 7), ("août", 8), ("aout", 8),
    ("septembre", 9), ("octobre", 10), ("novembre", 11), ("déc", 12), ("décembre", 12),
    // Spanish
    ("ene", 1), ("enero", 1), ("febrero", 2), ("marzo", 3), ("abr", 4), ("abril", 4), ("mayo", 5), ("junio", 6), ("julio", 7), ("ago", 8), ("agosto", 8), ("septiembre", 9), ("setiembre", 9),
    ("octubre", 10), ("noviembre", 11), ("dic", 12), ("diciembre", 12),
    // Italian
    ("gen", 1), ("gennaio", 1), ("febbraio", 2), ("aprile", 4), ("mag", 5), ("maggio", 5), ("giu", 6), ("giugno", 6), ("lug", 7), ("luglio", 7), ("set", 9), ("settembre", 9), ("ott", 10), ("ottobre", 10),
    ("dicembre", 12),
    // Portuguese
    ("janeiro", 1), ("fev", 2), ("fevereiro", 2), ("março", 3), ("maio", 5), ("junho", 6), ("julho", 7), ("setembro", 9), ("out", 10), ("outubro", 10), ("novembro", 11), ("dezembro", 12),
    // Dutch
    ("maart", 3), ("mrt", 3), ("mei", 5), ("augustus", 8),
    // Polish
    ("styczeń", 1), ("stycznia", 1), ("luty", 2), ("lutego", 2), ("marzec", 3), ("marca", 3), ("kwiecień", 4), ("kwietnia", 4), ("maja", 5), ("czerwiec", 6), ("czerwca", 6), ("lipiec", 7), ("lipca", 7),
    ("sierpień", 8), ("sierpnia", 8), ("wrzesień", 9), ("września", 9), ("październik", 10), ("października", 10), ("listopad", 11), ("listopada", 11), ("grudzień", 12), ("grudnia", 12),
    // Turkish
    ("ocak", 1), ("şubat", 2), ("mart", 3), ("nisan", 4), ("mayıs", 5), ("haziran", 6), ("temmuz", 7), ("ağustos", 8), ("eylül", 9), ("ekim", 10), ("kasım", 11), ("aralık", 12),
    // Russian (nominative and genitive)
    ("январь", 1), ("января", 1), ("февраль", 2), ("февраля", 2), ("март", 3), ("марта", 3), ("апрель", 4), ("апреля", 4), ("май", 5), ("мая", 5), ("июнь", 6), ("июня", 6), ("июль", 7), ("июля", 7),
    ("август", 8), ("августа", 8), ("сентябрь", 9), ("сентября", 9), ("октябрь", 10), ("октября", 10), ("ноябрь", 11), ("ноября", 11), ("декабрь", 12), ("декабря", 12),
];

/// «present», «heute», «aujourd'hui», «по настоящее время»: a job that has not ended.
const NOW_WORDS: &[&str] = &[
    "present", "current", "now", "today", "ongoing", "nå", "naa", "d.d.", "d.d", "dd", "pågående", "i dag", "dags dato", "heute", "aktuell", "jetzt", "bis heute", "laufend", "derzeit", "nu", "idag",
    "pågår", "nuværende", "nyt", "nykyhetki", "aujourd'hui", "aujourd’hui", "à ce jour", "actuel", "présent", "en cours", "actualidad", "actual", "presente", "hoy", "la fecha", "atual", "atualmente",
    "oggi", "attuale", "in corso", "heden", "huidig", "nu", "obecnie", "teraz", "do dziś", "současnost", "dosud", "nyní", "dabar", "prezent", "jelenleg", "napjainkig", "halen", "günümüz",
    "devam ediyor", "σήμερα", "по настоящее время", "по н.в.", "наст. время", "настоящее время", "сейчас", "н.в.", "дотепер", "тепер", "теперішній час", "sekarang", "saat ini", "hiện tại", "nay",
    "現在", "至今", "现在", "현재",
];

/// [`MONTH_NAMES`] and the months of assets/cv-languages.json.
static ALL_MONTHS: LazyLock<Vec<(&'static str, u32)>> = LazyLock::new(|| {
    let mut all = MONTH_NAMES.to_vec();
    let more: &'static [crate::headings::CvLanguage] = &crate::headings::MORE;
    for lang in more {
        for (i, names) in lang.months.iter().take(12).enumerate() {
            for n in names {
                let n = leak_lower(n.trim_end_matches('.'));
                // Very short or numeric forms («1», «m1») would read ordinary words as months.
                if n.chars().count() >= 2 && !n.chars().all(|c| c.is_ascii_digit()) && !all.iter().any(|(x, _)| *x == n) {
                    all.push((n, i as u32 + 1));
                }
            }
        }
    }
    all
});

/// [`NOW_WORDS`] and the «present» words of assets/cv-languages.json.
static ALL_NOW: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut all = NOW_WORDS.to_vec();
    let more: &'static [crate::headings::CvLanguage] = &crate::headings::MORE;
    for lang in more {
        for w in lang.now.iter().chain(std::iter::once(&lang.headings[8])) {
            let w = leak_lower(w);
            if !w.is_empty() && !all.contains(&w) {
                all.push(w);
            }
        }
    }
    all
});

fn alternation(words: impl Iterator<Item = &'static str>) -> String {
    let mut v: Vec<&str> = words.collect();
    // Longest first, so «september» wins over «sep».
    v.sort_by_key(|w| std::cmp::Reverse(w.chars().count()));
    v.iter().map(|w| regex::escape(w)).collect::<Vec<_>>().join("|")
}

/// A date: 03.2021, 2021-03, «März 2021», «Jan 2020», or a year.
static DATE: LazyLock<String> = LazyLock::new(|| {
    format!(r"(?:\d{{1,2}}\s*[./-]\s*(?:19|20)\d{{2}}|(?:19|20)\d{{2}}\s*[./-]\s*\d{{1,2}}|\b(?:{})\.?\s+(?:19|20)\d{{2}}|(?:19|20)\d{{2}})", alternation(ALL_MONTHS.iter().map(|(n, _)| *n)))
});
static NOW: LazyLock<String> = LazyLock::new(|| format!("(?:{})", alternation(ALL_NOW.iter().copied())));

/// A date range at the end of a line: «2019 – 2022», «03.2021 – nå», «Jan 2020 - Present», «2018 bis heute».
static RANGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)\(?\s*({date})\s*(?:–|—|-|−|to|til|till|bis|tot|à|au|a|al|hasta|até|ao|do|по|-)\s*({date}|{now})\s*\)?\s*$", date = *DATE, now = *NOW)).expect("regex")
});
/// «seit 03/2020», «since 2021», «depuis 2019»: a start and no end.
static SINCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)\(?\s*(?:seit|since|depuis|desde|dal|dall'|vanaf|sedan|siden|fra|od|с|от)\s+({})\s*\)?\s*$", *DATE)).expect("regex"));
/// One date or year at the end of a line: «… 2024».
static SINGLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)(?:^|[\s,·|(])({})\)?\s*$", *DATE)).expect("regex"));
static NOW_ALONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!("(?i)^{}$", *NOW)).expect("regex"));

/// The text before a date range at the end of the line, the start, and the end (`None` = now).
fn split_range(line: &str) -> Option<(String, String, Option<String>)> {
    if let Some(c) = RANGE.captures(line) {
        let whole = c.get(0)?;
        let before = clean_field(&line[..whole.start()]);
        let start = month_or_year(&c[1])?;
        let end = if checks::is_now(&c[2].replace('.', "")) || NOW_ALONE.is_match(c[2].trim()) { None } else { Some(month_or_year(&c[2])?) };
        return Some((before, start, end));
    }
    let c = SINCE.captures(line)?;
    let whole = c.get(0)?;
    Some((clean_field(&line[..whole.start()]), month_or_year(&c[1])?, None))
}

fn month_or_year(text: &str) -> Option<String> {
    let t = text.trim().trim_end_matches('.');
    // «März 2021» → «03.2021», which the form's date reader knows.
    let lower = t.to_lowercase();
    let named = lower.split_once(char::is_whitespace).and_then(|(m, y)| ALL_MONTHS.iter().find(|(n, _)| *n == m.trim_end_matches('.')).map(|(_, num)| format!("{num:02}.{}", y.trim())));
    let t = named.as_deref().unwrap_or(t);
    checks::parse_month(t).filter(|m| !m.is_empty()).or_else(|| checks::parse_year(t).filter(|y| !y.is_empty()))
}

fn clean_field(text: &str) -> String {
    text.trim().trim_end_matches([',', '|', '·', '–', '-', '—', ':', '(']).trim().to_string()
}

// ---------- lines ----------

const BULLETS: [char; 9] = ['•', '·', '▪', '◦', '‣', '–', '-', '*', '►'];

fn is_bullet(line: &str) -> bool {
    line.trim_start().starts_with(BULLETS)
}

fn unbullet(line: &str) -> &str {
    line.trim().trim_start_matches(BULLETS).trim()
}

/// A short line that can be an employer, a school or a title: a few words, not a sentence.
fn is_label(line: &str) -> bool {
    let t = line.trim();
    let words = t.split_whitespace().count();
    !t.is_empty() && !is_bullet(t) && (1..=if t.contains(" | ") { 18 } else { 9 }).contains(&words) && (!t.ends_with(['.', '!', '?', ';']) || is_company(t)) && heading_line(t).is_none()
}

/// A trailing place on an employer line: «Northstar Health London, UK», «Fjellvegg Bygg AS\tTrondheim».
fn split_place(line: &str, home: &str) -> (String, String) {
    static COUNTRYISH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+([A-ZÆØÅ][\p{L}.'\-]+,\s?[A-ZÆØÅ][\p{L}.\- ]{1,20})$").expect("regex"));
    let t = line.trim();
    // Most jobs are where the person lives: «… Civic Works Digital New York, NY».
    if !home.is_empty() && t.len() > home.len() + 2 && t.ends_with(home) {
        return (clean_field(&t[..t.len() - home.len()]), home.to_string());
    }
    for sep in ['\t', '|', '·'] {
        if let Some((a, b)) = t.rsplit_once(sep) {
            let (a, b) = (a.trim(), b.trim());
            if !a.is_empty() && !b.is_empty() && b.split_whitespace().count() <= 4 && !b.chars().any(|c| c.is_ascii_digit()) {
                return (a.to_string(), b.to_string());
            }
        }
    }
    if let Some((a, b)) = t.split_once("   ") {
        return (a.trim().to_string(), b.trim().to_string());
    }
    if let Some(c) = COUNTRYISH.captures(t) {
        let m = c.get(0).map(|m| m.start()).unwrap_or(t.len());
        return (t[..m].trim().to_string(), c[1].trim().to_string());
    }
    // The person's own town at the end of the line («… AS (fiktiv) Trondheim»).
    let town = home.split(',').next().unwrap_or("").trim();
    if !town.is_empty() && t.len() > town.len() + 2 && t.ends_with(town) {
        return (clean_field(&t[..t.len() - town.len()]), town.to_string());
    }
    (t.to_string(), String::new())
}

/// Bullet points from the lines under an entry. Lines that carry on a sentence are joined;
/// a Word file's list gives no bullet marks, so each finished sentence starts a new point.
fn bullets(lines: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let starts_new = is_bullet(t) || out.last().is_none_or(|last| last.ends_with(['.', '!', '?', ':']) || t.chars().next().is_some_and(char::is_uppercase) && !last.ends_with(','));
        let text = unbullet(t);
        if text.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(last) if !starts_new => {
                if last.ends_with('-') && !last.ends_with(" -") {
                    last.pop();
                    last.push_str(text);
                } else {
                    last.push(' ');
                    last.push_str(text);
                }
            }
            _ => out.push(text.to_string()),
        }
    }
    out
}

/// One dated entry in a section: the title on the date line (or a line by it), an employer or
/// school on the line before or after, and the lines under it.
#[derive(Debug, Default)]
struct Entry {
    on_date_line: String,
    labels: Vec<String>,
    start: String,
    end: Option<String>,
    body: Vec<String>,
}

/// Splits a section into dated entries. Lines before the first date belong to it (an employer
/// line above the title), and a short line right after a date line is its employer or school.
fn entries(lines: &[&str], max_above: usize) -> (Vec<Entry>, Vec<String>) {
    let lines: Vec<&str> = lines.iter().map(|l| l.trim()).collect();
    let dated: Vec<usize> =
        (0..lines.len()).filter(|&i| !is_bullet(lines[i]) && lines[i].split_whitespace().count() <= if lines[i].contains(" | ") { 26 } else { 14 } && split_range(lines[i]).is_some()).collect();
    if dated.is_empty() {
        return (Vec::new(), lines.iter().filter(|l| !l.is_empty()).map(|l| l.to_string()).collect());
    }
    let mut out = Vec::new();
    let mut used = vec![false; lines.len()];
    // Labels around each date line.
    let mut spans = Vec::new();
    for (k, &i) in dated.iter().enumerate() {
        let (before, start, end) = split_range(lines[i]).unwrap_or_default();
        used[i] = true;
        let mut e = Entry { on_date_line: before, start, end, ..Default::default() };
        // Up to two label lines right above, if they are not the previous entry's body.
        let floor = if k == 0 { 0 } else { dated[k - 1] + 1 };
        let mut above = Vec::new();
        let mut j = i;
        while j > floor && above.len() < max_above {
            j -= 1;
            if lines[j].is_empty() {
                if above.is_empty() {
                    continue;
                }
                break;
            }
            if !is_label(lines[j]) || used[j] {
                break;
            }
            // Under the previous entry, a short line just after its date line is that entry's employer.
            if k > 0 && j == dated[k - 1] + 1 {
                break;
            }
            above.push(j);
        }
        above.reverse();
        for &a in &above {
            used[a] = true;
            e.labels.push(lines[a].to_string());
        }
        // One label line right below.
        if let Some(n) = (i + 1..lines.len()).find(|&n| !lines[n].is_empty()) {
            let next_is_date = dated.get(k + 1) == Some(&n);
            // A line whose sentence carries on below («Mentored emerging designers / through …») is body, not a label.
            let carries_on = lines.get(n + 1).is_some_and(|l| l.trim_start().chars().next().is_some_and(char::is_lowercase));
            if !next_is_date && !carries_on && is_label(lines[n]) && !used[n] && (e.labels.is_empty() || e.on_date_line.is_empty() && e.labels.len() < 2) {
                // Not the next entry's label: that one sits right above the next date line.
                let next_date = dated.get(k + 1).copied().unwrap_or(lines.len());
                let only_labels_until_next = (n + 1..next_date).all(|m| lines[m].is_empty());
                if !(only_labels_until_next && next_date < lines.len()) {
                    used[n] = true;
                    e.labels.push(lines[n].to_string());
                }
            }
        }
        spans.push(e);
    }
    // Bodies: the unused lines up to the next entry.
    let starts: Vec<usize> = dated.clone();
    for (k, mut e) in spans.into_iter().enumerate() {
        let from = starts[k] + 1;
        let to = starts.get(k + 1).copied().unwrap_or(lines.len());
        e.body = (from..to).filter(|&m| !used[m] && !lines[m].is_empty()).map(|m| lines[m].to_string()).collect();
        // The next entry's label lines were taken above; anything left between is body.
        for m in from..to {
            used[m] = true;
        }
        out.push(e);
    }
    let before_first: Vec<String> = (0..dated[0]).filter(|&m| !used[m] && !lines[m].is_empty()).map(|m| lines[m].to_string()).collect();
    (out, before_first)
}

/// A line that names a company by its legal form: «Fjellvegg Bygg AS», «Beispiel GmbH», «Acme Ltd».
fn is_company(line: &str) -> bool {
    const FORMS: &[&str] = &[
        "as", "asa", "ab", "aps", "a/s", "oy", "oyj", "gmbh", "ag", "kg", "se", "sas", "sa", "sarl", "s.a.", "s.l.", "sl", "srl", "s.r.l.", "spa", "s.p.a.", "bv", "b.v.", "nv", "ltd", "ltd.", "limited",
        "inc", "inc.", "llc", "plc", "corp", "corp.", "co.", "lda", "ltda", "sp.", "z", "o.o.", "s.r.o.", "kft", "zrt", "a.ş.", "ooo", "ооо", "pt", "tbk", "kk", "株式会社", "有限公司",
    ];
    let lower = line.to_lowercase();
    let first_part = lower.split(',').next().unwrap_or("");
    first_part.split_whitespace().any(|w| FORMS.contains(&w.trim_matches(|c: char| c == ',' || c == '(' || c == ')')))
}

/// Splits «Senior Designer, Acme» or «Konsulent hos Acme» into title and employer.
fn title_and_employer(text: &str) -> (String, String) {
    for sep in [" hos ", " at ", " @ ", " bei ", " chez ", " presso ", " bij ", " på ", " – ", " — ", " | ", ", ", " - "] {
        if let Some((a, b)) = text.split_once(sep) {
            if !a.trim().is_empty() && !b.trim().is_empty() {
                return (a.trim().to_string(), b.trim().to_string());
            }
        }
    }
    // «Analista de datos en Ejemplo S.L.»: words that also mean other things split only before a company.
    for sep in [" en ", " em ", " na ", " no ", " w ", " в ", " в компании ", " di ", " tại "] {
        if let Some((a, b)) = text.rsplit_once(sep) {
            if !a.trim().is_empty() && is_company(b) {
                return (a.trim().to_string(), b.trim().to_string());
            }
        }
    }
    (text.to_string(), String::new())
}

fn read_jobs(lines: &[&str], home: &str) -> (Vec<Job>, Vec<String>) {
    let (entries, loose) = entries(lines, 2);
    let mut jobs = Vec::new();
    for (n, e) in entries.into_iter().enumerate() {
        let mut title = e.on_date_line.clone();
        let mut employer_line = String::new();
        let mut labels = e.labels.clone();
        if title.is_empty() && !labels.is_empty() {
            // Two lines over a date: the one that names a company («… GmbH», «… SAS») is the employer.
            if labels.len() == 2 && is_company(&labels[1]) && !is_company(&labels[0]) {
                labels.swap(0, 1);
            }
            title = labels.pop().unwrap_or_default();
        }
        if let Some(l) = labels.first() {
            employer_line = l.clone();
        }
        let (mut company, mut location) = split_place(&employer_line, home);
        if company.is_empty() {
            let (t, c) = title_and_employer(&title);
            if !c.is_empty() {
                title = t;
                let (c, l) = split_place(&c, home);
                company = c;
                location = l;
            }
        }
        let body: Vec<&str> = e.body.iter().map(String::as_str).collect();
        jobs.push(Job { id: format!("job_imported_{}", n + 1), company, title, location, start: e.start, end: e.end, bullets: bullets(&body), evidence: String::new(), confirmed: true });
    }
    (jobs, loose)
}

const SCHOOL_WORDS: &[&str] = &[
    "univers",
    "université",
    "hochschule",
    "college",
    "høgskole",
    "høgskulen",
    "høyskole",
    "høyskolen",
    "school",
    "skole",
    "skolen",
    "institute",
    "institutt",
    "academy",
    "akademi",
    "akademiet",
    "fagskole",
    "fagskolen",
    "gymnas",
    "videregående",
    "polytechnic",
    "business school",
    "ntnu",
    "uio",
    "uib",
    "oslomet",
    "nmbu",
    "uit",
    "bi ",
];
const DEGREES: &[&str] =
    &["BA", "BSc", "BS", "BBA", "BEng", "BEd", "LLB", "MA", "MSc", "MS", "MBA", "MEng", "MEd", "LLM", "MPhil", "PhD", "DPhil", "Bachelor", "Master", "Cand.", "Fagbrev", "Svennebrev"];

/// A line that is a degree, or starts with one: «MA», «BSc Computer Science».
fn is_degree(text: &str) -> bool {
    text.split_whitespace()
        .next()
        .is_some_and(|w| DEGREES.iter().chain(MORE_DEGREES).any(|d| d.to_lowercase() == w.trim_end_matches([',']).to_lowercase() || d.eq_ignore_ascii_case(w.trim_end_matches([',', '.']))))
}

fn is_school(text: &str) -> bool {
    let l = format!("{} ", text.to_lowercase());
    SCHOOL_WORDS.iter().chain(MORE_SCHOOL_WORDS).any(|w| l.contains(w))
}

/// Schools and degrees in the other languages.
const MORE_SCHOOL_WORDS: &[&str] = &[
    "fachhochschule", "schule", "gymnasium", "berufsschule", "akademie", "école", "lycée", "institut", "escuela", "instituto", "colegio", "facultad", "scuola", "istituto", "politecnico", "liceo",
    "escola", "faculdade", "hogeschool", "school", "gymnasie", "högskola", "folkhögskola", "gymnasiet", "skole", "erhvervsakademi", "seminarium", "lukio", "ammattikorkeakoulu", "yliopisto", "opisto",
    "szkoła", "politechnika", "akademia", "liceum", "škola", "vysoká", "gimnázium", "egyetem", "főiskola", "iskola", "üniversite", "lise", "okul", "yüksekokul", "fakülte", "университет", "институт",
    "колледж", "школа", "академия", "техникум", "університет", "коледж", "πανεπιστήμιο", "σχολή", "ίδρυμα", "universitas", "institut teknologi", "politeknik", "sekolah", "đại học", "trường", "học viện",
    "大学", "学院", "大學", "대학교", "大学院",
];
const MORE_DEGREES: &[&str] = &[
    "Diplom", "Dipl.-Ing.", "B.Sc.", "M.Sc.", "B.A.", "M.A.", "Magister", "Staatsexamen", "Ausbildung", "Licence", "Maîtrise", "BTS", "DUT", "Grado", "Licenciatura", "Licenciado", "Máster", "Mestrado",
    "Laurea", "Diploma", "Kandidat", "Kandidatexamen", "Kandidatgrad", "Magisterexamen", "Kandidaatti", "Maisteri", "Tradenomi", "Insinööri", "Licencjat", "Magister", "Inżynier", "Inżynierski",
    "Mgr.", "Bc.", "Ing.", "Licență", "Diplomă", "Lisans", "Yüksek", "Ön", "Бакалавр", "Магистр", "Специалист", "Магістр", "Πτυχίο", "Μεταπτυχιακό", "Sarjana", "Cử", "Thạc", "Kỹ",
];

fn read_education(lines: &[&str]) -> (Vec<Education>, Vec<String>) {
    let (entries, loose) = entries(lines, 5);
    let mut out = Vec::new();
    for e in entries {
        let mut parts: Vec<String> = e.labels.clone();
        if !e.on_date_line.is_empty() {
            parts.extend(e.on_date_line.split(" | ").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
        }
        // A short line under the date that is a school («Eksempel høgskole») belongs to the entry.
        let mut body = e.body.clone();
        if let Some(first) = body.first().cloned() {
            if is_label(&first) && (is_school(&first) || parts.len() < 2) {
                parts.push(first);
                body.remove(0);
            }
        }
        let mut institution = String::new();
        let mut qualification = Vec::new();
        // «University of the Arts / London / MA / Brand Communication»: the school, then the degree.
        if let Some(d) = parts.iter().position(|p| is_degree(p)).filter(|&d| d > 0 && parts[..d].iter().any(|p| is_school(p))) {
            let start = checks::parse_year(&e.start).unwrap_or_default();
            let end = e.end.as_deref().and_then(|v| checks::parse_year(v)).unwrap_or_default();
            out.push(Education { qualification: parts[d..].join(" "), institution: parts[..d].join(" "), start, end });
            continue;
        }
        for p in &parts {
            if institution.is_empty() && is_school(p) {
                // «University of Brighton BA»: the degree at the end goes to the qualification.
                let mut words: Vec<&str> = p.split_whitespace().collect();
                if let Some(last) = words.last() {
                    if DEGREES.iter().any(|d| d.eq_ignore_ascii_case(last.trim_end_matches(','))) {
                        qualification.insert(0, last.to_string());
                        words.pop();
                    }
                }
                institution = words.join(" ");
            } else {
                qualification.push(p.clone());
            }
        }
        if institution.is_empty() && qualification.len() >= 2 {
            institution = qualification.pop().unwrap_or_default();
        }
        let start = checks::parse_year(&e.start).unwrap_or_default();
        let end = e.end.as_deref().and_then(|v| checks::parse_year(v)).unwrap_or_default();
        out.push(Education { qualification: qualification.join(" "), institution, start, end });
    }
    (out, loose)
}

fn read_skills(lines: &[&str]) -> Vec<Skill> {
    // A line ending with a comma carries on below.
    let mut joined: Vec<String> = Vec::new();
    for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        match joined.last_mut() {
            Some(last) if last.ends_with(',') || l.chars().next().is_some_and(char::is_lowercase) => {
                last.push(' ');
                last.push_str(l);
            }
            _ => joined.push(l.to_string()),
        }
    }
    let mut out = Vec::new();
    let mut group = String::new();
    for (i, line) in joined.iter().enumerate() {
        let text = unbullet(line);
        // «Prosjekt: fremdriftsplaner, innkjøp …»
        if let Some((g, rest)) = text.split_once(':') {
            if g.split_whitespace().count() <= 4 && !rest.trim().is_empty() {
                group = g.trim().to_string();
                push_skills(&mut out, &group, rest);
                continue;
            }
        }
        // A short line without commas over a list is the list's group name.
        let next_is_list = joined.get(i + 1).is_some_and(|n| n.contains(','));
        if !text.contains(',') && text.split_whitespace().count() <= 4 && next_is_list {
            group = text.to_string();
            continue;
        }
        push_skills(&mut out, &group, text);
    }
    out
}

fn push_skills(out: &mut Vec<Skill>, group: &str, text: &str) {
    for item in text.split([',', '•', '·', ';', '|']).map(|s| s.trim().trim_start_matches(['-', '*', '–']).trim()) {
        if (2..=60).contains(&item.chars().count()) && !out.iter().any(|s: &Skill| s.name.eq_ignore_ascii_case(item)) {
            out.push(Skill { group: group.to_string(), name: item.to_string(), evidence: String::new(), confirmed: true });
        }
    }
}

const LEVELS: &[&str] = &[
    "native",
    "fluent",
    "conversational",
    "basic",
    "beginner",
    "elementary",
    "intermediate",
    "advanced",
    "proficient",
    "professional",
    "bilingual",
    "good",
    "working",
    "limited",
    "full",
    "morsmål",
    "flytende",
    "god",
    "gode",
    "godt",
    "grunnleggende",
    "muntlig",
    "skriftlig",
    "middels",
    "noe",
    "litt",
    "a1",
    "a2",
    "b1",
    "b2",
    "c1",
    "c2",
];

fn is_level(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    LEVELS.contains(&w.as_str()) || MORE_LEVELS.contains(&w.as_str())
}

/// Language levels in the other languages.
const MORE_LEVELS: &[&str] = &[
    "muttersprache", "fließend", "fliessend", "verhandlungssicher", "gut", "sehr", "grundkenntnisse", "fortgeschritten", "konversationssicher", "maternelle", "courant", "bilingue", "notions",
    "intermédiaire", "avancé", "scolaire", "nativo", "nativa", "fluido", "avanzado", "básico", "intermedio", "bilingüe", "alto", "medio", "madrelingua", "fluente", "ottimo", "buono", "base",
    "avanzato", "intermedio", "nativo", "materna", "avançado", "básico", "moedertaal", "vloeiend", "goed", "basis", "modersmål", "flytande", "grundläggande", "flydende", "modersmål", "äidinkieli",
    "sujuva", "hyvä", "perustaso", "tyydyttävä", "ojczysty", "biegły", "biegle", "dobry", "podstawowy", "komunikatywny", "rodilý", "plynně", "pokročilý", "základní", "nativ", "fluent", "anyanyelv",
    "anyanyelvi", "felsőfok", "középfok", "alapfok", "folyékony", "anadil", "akıcı", "ileri", "orta", "başlangıç", "iyi", "родной", "свободно", "свободный", "свободное", "базовый",
    "разговорный", "средний", "продвинутый", "рідна", "вільно", "вільний", "розмовний", "середній", "μητρική", "άριστα", "καλά", "πολύ", "ασταθής", "asli", "lancar", "baik", "dasar", "menengah",
    "bản", "thông", "thạo", "lưu", "loát", "cơ", "母語", "ネイティブ", "ビジネスレベル", "日常会話", "母语", "流利", "熟练", "原生", "모국어", "능통", "유창",
];

/// Language names in English and Norwegian, so a language list cannot swallow other text.
const LANGUAGE_NAMES: &[&str] = &[
    "norwegian",
    "norsk",
    "bokmål",
    "nynorsk",
    "english",
    "engelsk",
    "swedish",
    "svensk",
    "danish",
    "dansk",
    "finnish",
    "finsk",
    "icelandic",
    "islandsk",
    "german",
    "tysk",
    "french",
    "fransk",
    "spanish",
    "spansk",
    "portuguese",
    "portugisisk",
    "italian",
    "italiensk",
    "dutch",
    "nederlandsk",
    "polish",
    "polsk",
    "czech",
    "tsjekkisk",
    "slovak",
    "slovakisk",
    "hungarian",
    "ungarsk",
    "romanian",
    "rumensk",
    "bulgarian",
    "bulgarsk",
    "greek",
    "gresk",
    "turkish",
    "tyrkisk",
    "russian",
    "russisk",
    "ukrainian",
    "ukrainsk",
    "lithuanian",
    "litauisk",
    "latvian",
    "latvisk",
    "estonian",
    "estisk",
    "serbian",
    "serbisk",
    "croatian",
    "kroatisk",
    "bosnian",
    "bosnisk",
    "slovenian",
    "slovensk",
    "albanian",
    "albansk",
    "arabic",
    "arabisk",
    "hebrew",
    "hebraisk",
    "persian",
    "farsi",
    "persisk",
    "urdu",
    "hindi",
    "punjabi",
    "bengali",
    "tamil",
    "chinese",
    "kinesisk",
    "mandarin",
    "cantonese",
    "kantonesisk",
    "japanese",
    "japansk",
    "korean",
    "koreansk",
    "vietnamese",
    "vietnamesisk",
    "thai",
    "thailandsk",
    "tagalog",
    "filipino",
    "indonesian",
    "indonesisk",
    "malay",
    "malayisk",
    "somali",
    "somalisk",
    "tigrinya",
    "tigrinja",
    "amharic",
    "amharisk",
    "swahili",
    "kurdish",
    "kurdisk",
    "pashto",
    "dari",
    "sami",
    "samisk",
    "nordsamisk",
    "kven",
    "kvensk",
    "sign language",
    "tegnspråk",
    "norsk tegnspråk",
    "catalan",
    "katalansk",
    "irish",
    "irsk",
    "welsh",
    "walisisk",
    "afrikaans",
];

fn is_language_name(text: &str) -> bool {
    let l = text.trim().to_lowercase();
    LANGUAGE_NAMES.contains(&l.as_str()) || MORE_LANGUAGE_NAMES.contains(&l.as_str())
}

/// Language names as the other languages write them («Englisch», «anglais», «inglés» …).
const MORE_LANGUAGE_NAMES: &[&str] = &[
    // German
    "deutsch", "englisch", "französisch", "spanisch", "italienisch", "portugiesisch", "niederländisch", "norwegisch", "schwedisch", "dänisch", "finnisch", "polnisch", "russisch", "türkisch",
    "arabisch", "chinesisch", "japanisch", "koreanisch", "griechisch", "ungarisch", "tschechisch", "rumänisch", "ukrainisch",
    // French
    "français", "anglais", "allemand", "espagnol", "italien", "portugais", "néerlandais", "norvégien", "suédois", "danois", "finnois", "polonais", "russe", "turc", "arabe", "chinois", "japonais",
    "coréen", "grec", "hongrois",
    // Spanish, Portuguese, Italian
    "español", "castellano", "inglés", "alemán", "francés", "italiano", "portugués", "neerlandés", "noruego", "sueco", "danés", "polaco", "ruso", "turco", "árabe", "chino", "japonés", "catalán",
    "português", "inglês", "alemão", "francês", "espanhol", "holandês", "norueguês", "inglese", "tedesco", "francese", "spagnolo", "portoghese", "olandese", "norvegese", "svedese", "russo",
    // Dutch, Swedish, Danish, Finnish
    "nederlands", "engels", "duits", "frans", "spaans", "italiaans", "noors", "zweeds", "deens", "svenska", "engelska", "tyska", "franska", "spanska", "norska", "danska", "finska", "italienska",
    "dansk", "tysk", "fransk", "spansk", "svensk", "suomi", "englanti", "ruotsi", "saksa", "ranska", "espanja", "norja", "venäjä", "viro",
    // Polish, Czech, Romanian, Hungarian, Turkish
    "polski", "angielski", "niemiecki", "francuski", "hiszpański", "rosyjski", "włoski", "ukraiński", "čeština", "angličtina", "němčina", "francouzština", "ruština", "slovenština", "română",
    "engleză", "germană", "franceză", "spaniolă", "italiană", "magyar", "angol", "német", "francia", "spanyol", "orosz", "türkçe", "ingilizce", "almanca", "fransızca", "ispanyolca", "arapça",
    "rusça", "kürtçe",
    // Russian, Ukrainian, Greek
    "русский", "английский", "немецкий", "французский", "испанский", "итальянский", "украинский", "казахский", "українська", "англійська", "німецька", "французька", "польська", "російська",
    "ελληνικά", "αγγλικά", "γερμανικά", "γαλλικά", "ιταλικά", "ισπανικά",
    // Indonesian, Vietnamese, Japanese, Chinese, Korean
    "bahasa indonesia", "indonesia", "inggris", "jerman", "prancis", "mandarin", "jepang", "tiếng việt", "tiếng anh", "tiếng pháp", "tiếng đức", "tiếng trung", "tiếng nhật", "日本語", "英語",
    "中国語", "韓国語", "中文", "汉语", "英语", "日语", "韩语", "한국어", "영어", "일본어", "중국어",
];

fn read_languages(lines: &[&str]) -> Vec<SpokenLanguage> {
    let text = lines.join("\n");
    let mut out: Vec<SpokenLanguage> = Vec::new();
    let items: Vec<&str> = text.split(['\n', '·', ',', ';', '|', '•']).map(str::trim).filter(|s| !s.is_empty()).collect();
    // Languages that share the next one's level («Norwegian and / English fluent»), over lines.
    let mut shared: Vec<usize> = Vec::new();
    let share = |out: &mut Vec<SpokenLanguage>, shared: &[usize]| {
        for &i in shared.iter().rev() {
            if let Some(level) = out.get(i + 1).map(|l| l.level.clone()) {
                if out[i].level.is_empty() {
                    out[i].level = level;
                }
            }
        }
    };
    for item in items {
        if let Some((name, level)) = item.split_once([':', '(']).or_else(|| item.split_once(" – ")).or_else(|| item.split_once(" - ")) {
            let level = level.trim().trim_end_matches(')').trim();
            // «Norsk og engelsk: flytende» gives two languages with the same level.
            let names: Vec<&str> = name.split(" og ").flat_map(|n| n.split(" and ")).map(str::trim).collect();
            if names.iter().all(|n| is_language_name(n)) {
                for n in names {
                    out.push(SpokenLanguage { name: capitalise(n), level: level.to_string() });
                }
            }
            continue;
        }
        // «English Fluent Dutch Conversational» from a two-column PDF: language, then its level.
        // «Norwegian and English fluent written and spoken»: both languages share the level.
        let words: Vec<&str> = item.split_whitespace().collect();
        let mut k = 0;
        while k < words.len() {
            let word = words[k];
            let bare = word.trim_matches(|c: char| !c.is_alphabetic());
            let is_and = matches!(bare.to_lowercase().as_str(), "and" | "og") || word == "&";
            // The next language may be on the next line.
            let next_is_language = words.get(k + 1).is_none_or(|w| is_language_name(w.trim_matches(|c: char| !c.is_alphabetic())));
            if is_and && next_is_language && out.last().is_some_and(|l| l.level.is_empty()) {
                shared.push(out.len() - 1);
            } else if is_language_name(bare) {
                out.push(SpokenLanguage { name: capitalise(bare), level: String::new() });
            } else if is_level(word) || !word.chars().any(char::is_uppercase) && out.last().is_some_and(|l| !l.level.is_empty() && l.level.split_whitespace().count() < 5) {
                // A level, or the rest of one («written and spoken»).
                if let Some(last) = out.last_mut() {
                    if !last.level.is_empty() {
                        last.level.push(' ');
                    }
                    last.level.push_str(word);
                }
            } else {
                // Not a language: this is not a language list (a two-column layout mixed it up).
                break;
            }
            k += 1;
        }
        if k < words.len() {
            share(&mut out, &shared);
            return out;
        }
    }
    share(&mut out, &shared);
    out
}

fn capitalise(word: &str) -> String {
    let mut c = word.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn read_certifications(lines: &[&str]) -> Vec<Certification> {
    // Entries are split by blank lines, or each line is one when every line has a year.
    let mut groups: Vec<Vec<&str>> = vec![Vec::new()];
    for l in lines {
        if l.trim().is_empty() {
            if groups.last().is_some_and(|g| !g.is_empty()) {
                groups.push(Vec::new());
            }
        } else if let Some(g) = groups.last_mut() {
            g.push(l.trim());
        }
    }
    groups.retain(|g| !g.is_empty());
    if groups.len() == 1 && groups[0].len() > 1 && groups[0].iter().all(|l| SINGLE.is_match(l) || l.contains('·')) {
        groups = groups[0].iter().map(|l| vec![*l]).collect();
    }
    let mut out = Vec::new();
    for g in groups {
        let text = g.join("\n");
        // «Prosjektledelse, grunnkurs · Eksempel kursleverandør · 2022»
        let year_at = g.iter().position(|l| SINGLE.is_match(l));
        let (name, mut issuer, year);
        if let Some(i) = year_at {
            let line = g[i];
            let c = SINGLE.captures(line).expect("matched");
            year = checks::parse_year(&c[1]).unwrap_or_default();
            let before_year = clean_field(&line[..c.get(0).map(|m| m.start()).unwrap_or(0)]);
            let mut head: Vec<String> = g[..i].iter().map(|s| s.to_string()).collect();
            if !before_year.is_empty() {
                head.push(before_year);
            }
            let head = head.join(" ");
            let mut pieces = head.split(['·', '|']).map(str::trim).filter(|s| !s.is_empty());
            name = pieces.next().unwrap_or("").to_string();
            issuer = pieces.collect::<Vec<_>>().join(", ");
            if issuer.is_empty() {
                issuer = g[i + 1..].join(" ");
            }
        } else {
            let mut pieces = text.split(['·', '|', '\n']).map(str::trim).filter(|s| !s.is_empty());
            name = pieces.next().unwrap_or("").to_string();
            issuer = pieces.collect::<Vec<_>>().join(", ");
            year = String::new();
        }
        if (2..=120).contains(&name.chars().count()) {
            out.push(Certification { name, issuer, year });
        }
    }
    out
}

fn read_interests(lines: &[&str]) -> Vec<Interest> {
    let mut out = Vec::new();
    for item in lines.join("\n").split(['\n', '·', ',', ';', '•', '|']).map(|s| unbullet(s).trim()) {
        if (2..=60).contains(&item.chars().count()) {
            out.push(Interest { id: format!("interest_imported_{}", out.len() + 1), label: item.to_string(), category: "other".into(), specificity: 1, confirmed: true, ..Default::default() });
        }
    }
    out
}

fn read_custom(lines: &[&str], id: &str, title: &str) -> Option<CustomSection> {
    let mut items = Vec::new();
    // Dated items first; then single-year lines («Open Metrics Toolkit 2025»).
    let (entries, loose) = entries(lines, 2);
    for e in entries {
        let (title, subtitle) = if e.on_date_line.is_empty() {
            (e.labels.first().cloned().unwrap_or_default(), e.labels.get(1).cloned().unwrap_or_default())
        } else {
            (e.on_date_line.clone(), e.labels.first().cloned().unwrap_or_default())
        };
        let end = e.end.as_deref().map(checks::show_month).unwrap_or_else(|| "nå".to_string());
        let body: Vec<&str> = e.body.iter().map(String::as_str).collect();
        items.push(CustomItem { title, subtitle, date: format!("{} – {end}", checks::show_month(&e.start)), bullets: bullets(&body) });
    }
    let mut current: Option<CustomItem> = None;
    for line in &loose {
        let single = (!is_bullet(line) && line.split_whitespace().count() <= 12).then(|| SINGLE.captures(line)).flatten();
        if let Some(c) = single {
            if let Some(item) = current.take() {
                items.push(item);
            }
            let at = c.get(0).map(|m| m.start()).unwrap_or(0);
            current = Some(CustomItem { title: clean_field(&line[..at]), date: checks::parse_year(&c[1]).unwrap_or_default(), ..Default::default() });
        } else if let Some(item) = current.as_mut() {
            let mut b = item.bullets.clone();
            b.push(line.clone());
            let refs: Vec<&str> = b.iter().map(String::as_str).collect();
            item.bullets = bullets(&refs);
        } else {
            current = Some(CustomItem { title: unbullet(line).to_string(), ..Default::default() });
        }
    }
    if let Some(item) = current {
        items.push(item);
    }
    items.retain(|i| !i.title.is_empty() || !i.bullets.is_empty());
    (!items.is_empty()).then(|| CustomSection { id: id.to_string(), title: title.to_string(), items })
}

// ---------- the top of the CV ----------

static EMAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("regex"));
static PHONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:\+\d{1,3}[\s.\-]?)?(?:\(\d{1,4}\)[\s.\-]?)?(?:\d[\s.\-]?){6,11}\d").expect("regex"));
static LINKEDIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(?:https?://)?(?:[a-z]{2,3}\.)?linkedin\.com/in/[A-Za-z0-9_\-%]+/?").expect("regex"));
static WEB: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:https?://)?(?:www\.)?[a-z0-9\-]+(?:\.[a-z0-9\-]+)*\.(?:com|no|net|org|io|dev|me|se|dk|uk|de|app|page|site|info|co)(?:/[^\s·|,]*)?").expect("regex"));

/// Words that make a short line a job title rather than a name.
const TITLE_WORDS: &[&str] = &[
    "senior",
    "junior",
    "lead",
    "head",
    "chief",
    "manager",
    "designer",
    "engineer",
    "developer",
    "consultant",
    "coordinator",
    "specialist",
    "analyst",
    "director",
    "officer",
    "assistant",
    "advisor",
    "adviser",
    "architect",
    "scientist",
    "nurse",
    "teacher",
    "accountant",
    "administrator",
    "technician",
    "rådgiver",
    "leder",
    "konsulent",
    "utvikler",
    "ingeniør",
    "koordinator",
    "medarbeider",
    "sykepleier",
    "lærer",
    "daglig",
    "prosjektleder",
    "selger",
    "tekniker",
    "operatør",
];

fn looks_like_name(line: &str) -> bool {
    let t = line.trim();
    let words = t.split_whitespace().count();
    (2..=5).contains(&words) && !t.contains('@') && !t.chars().any(|c| c.is_ascii_digit()) && heading_line(t).is_none() && t.chars().all(|c| c.is_alphabetic() || " -.'".contains(c))
}

fn has_title_word(line: &str) -> bool {
    line.to_lowercase().split_whitespace().any(|w| TITLE_WORDS.contains(&w))
}

/// A page number or running header: «1 / 2», «Side 1 av 2», «Ingrid Haugland 1 / 1», «VebjørnJohannessen1/2».
fn is_page_mark(line: &str) -> bool {
    static COUNTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^([\p{L} .'\-]{0,60}?)\s*(?:side|page|s\.)?\s*\d{1,2}\s*(?:/|av|of)\s*\d{1,2}\s*$").expect("regex"));
    COUNTER.captures(line.trim()).is_some_and(|c| c[1].split_whitespace().count() <= 5)
}

/// «LUCAS MARTINS» → «Lucas Martins»; names already in mixed case are kept.
fn name_case(name: &str) -> String {
    if name.chars().any(char::is_lowercase) {
        return name.to_string();
    }
    name.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_contact_line(line: &str) -> bool {
    EMAIL.is_match(line)
        || PHONE.find_iter(line).any(|m| m.as_str().chars().filter(char::is_ascii_digit).count() >= 8)
        || LINKEDIN.is_match(line)
        || WEB.is_match(line) && line.split_whitespace().count() <= 6
}

/// The town on a contact line once the e-mail, phone and links are taken out.
fn town_from(line: &str) -> Option<String> {
    let mut rest = line.to_string();
    for re in [&*EMAIL, &*LINKEDIN, &*WEB, &*PHONE] {
        rest = re.replace_all(&rest, " | ").to_string();
    }
    rest.split(['|', '·', '•', '\t'])
        .map(str::trim)
        .find(|s| {
            let n = s.chars().count();
            (2..=40).contains(&n) && s.chars().all(|c| c.is_alphabetic() || " ,.-'".contains(c)) && s.chars().next().is_some_and(char::is_uppercase) && s.split_whitespace().count() <= 4
        })
        .map(|s| s.trim_end_matches(',').to_string())
}

/// Reads an existing CV's text without AI.
pub fn read_cv(text: &str) -> CvRead {
    let text = tidy(text);
    let mut lines: Vec<String> = text.lines().map(unspace).filter(|l| !is_page_mark(l)).collect();
    let mut p = Project::default();
    let mut found: Vec<&str> = Vec::new();

    // Contact details anywhere near the top (two-column PDFs put them in odd places).
    let c = &mut p.candidate;
    if let Some(m) = EMAIL.find(&text) {
        c.email = m.as_str().to_string();
        found.push("e-post");
    }
    if let Some(m) = PHONE.find_iter(&text).find(|m| m.as_str().chars().filter(char::is_ascii_digit).count() >= 8 && !m.as_str().contains('.') && !RANGE.is_match(m.as_str())) {
        c.phone = m.as_str().trim().to_string();
        found.push("telefon");
    }
    if let Some(m) = LINKEDIN.find(&text) {
        let url = m.as_str().trim_end_matches('/');
        c.linkedin = if url.starts_with("http") { url.to_string() } else { format!("https://{url}") };
        found.push("LinkedIn");
    }
    // Lines with the e-mail, phone or LinkedIn, wherever the layout put them, and links right by them.
    let personal = |l: &str| EMAIL.is_match(l) || LINKEDIN.is_match(l) || PHONE.find_iter(l).any(|m| m.as_str().chars().filter(char::is_ascii_digit).count() >= 8 && !RANGE.is_match(l));
    let anchors: Vec<usize> = (0..lines.len()).filter(|&i| personal(&lines[i]) && heading_line(&lines[i]).is_none_or(|(_, rest)| rest.is_empty())).collect();
    let contact_lines: Vec<usize> =
        (0..lines.len()).filter(|&i| anchors.contains(&i) || is_contact_line(&lines[i]) && lines[i].split_whitespace().count() <= 6 && anchors.iter().any(|&a| a.abs_diff(i) <= 2)).collect();
    let near_contact: Vec<usize> = match (contact_lines.first(), contact_lines.last()) {
        (Some(&a), Some(&b)) => (a.saturating_sub(2)..(b + 3).min(lines.len())).collect(),
        _ => Vec::new(),
    };
    let town_line = |i: &usize| {
        let l = lines[*i].trim();
        let alone = l.contains(',') && l.split_whitespace().count() <= 4 && l.chars().all(|c| c.is_alphabetic() || " ,.-'".contains(c));
        if contact_lines.contains(i) || alone { town_from(l) } else { None }
    };
    let mut taken: Vec<usize> = contact_lines.clone();
    if let Some((i, town)) = near_contact.iter().find_map(|i| town_line(i).map(|t| (*i, t))) {
        taken.push(i);
        c.city = town;
        found.push("sted");
    }
    if let Some(web) = contact_lines.iter().find_map(|&i| {
        WEB.find_iter(&lines[i]).map(|m| m.as_str().to_string()).find(|w| !w.to_lowercase().contains("linkedin") && !c.email.ends_with(w.trim_start_matches("www.")) && !c.email.contains(w.as_str()))
    }) {
        c.extra.push(ContactField { id: "web".into(), label: String::new(), value: web });
        found.push("nettside");
    }

    // The name: a short line near the top; a job title above it («SENIOR CONTENT DESIGNER») is skipped.
    let first_contact = contact_lines.first().copied().unwrap_or(lines.len().min(10));
    let near: Vec<usize> = (0..lines.len().min(12)).filter(|&i| looks_like_name(&lines[i])).collect();
    // In a two-column PDF the name can come after a sidebar: a name line among the few lines just
    // above the contact lines wins.
    let above_contact: Vec<usize> = (0..first_contact).rev().filter(|&i| !lines[i].trim().is_empty()).take(3).collect();
    let by_contact = above_contact.iter().copied().filter(|&i| looks_like_name(&lines[i]) && !has_title_word(&lines[i])).max();
    let name_at = by_contact.or(match near.as_slice() {
        [a, b, ..] if has_title_word(&lines[*a]) && !has_title_word(&lines[*b]) => Some(*b),
        [a, ..] => Some(*a),
        [] => (first_contact.saturating_sub(3)..first_contact).rev().find(|&i| looks_like_name(&lines[i]) && !has_title_word(&lines[i])),
    });
    if let Some(i) = name_at {
        c.name = name_case(lines[i].trim());
        found.push("navn");
        taken.push(i);
        // The headline: the short line under the name, or a title line just above it.
        let below = (i + 1..lines.len().min(i + 3)).find(|&j| !lines[j].trim().is_empty());
        let above = i.checked_sub(1).filter(|&j| has_title_word(&lines[j]) && looks_like_name(&lines[j]));
        if let Some(j) = below.filter(|&j| is_label(&lines[j]) && !is_contact_line(&lines[j]) && !lines[j].chars().any(|c| c.is_ascii_digit())).or(above) {
            c.headline = name_case(lines[j].trim());
            found.push("tittel");
            taken.push(j);
        }
    }
    // A page header repeats the name and title on every page; those lines are not part of a section.
    let (name, headline) = (p.candidate.name.to_lowercase(), p.candidate.headline.to_lowercase());
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim().to_lowercase();
        if !t.is_empty() && (t == name || !headline.is_empty() && t == headline) {
            taken.push(i);
        }
    }
    // The name, title and contact lines are read; they are not part of any section below.
    for i in taken {
        if let Some(l) = lines.get_mut(i) {
            l.clear();
        }
    }

    // Sections.
    let mut parts: Vec<(Part, Vec<String>)> = vec![(Part::Top, Vec::new())];
    // The language of each heading: a German CV has German headings, and gets them back.
    let mut heading_titles: Vec<String> = vec![String::new()];
    let (mut ours, mut theirs): (usize, Vec<&'static str>) = (0, Vec::new());
    for line in &lines {
        if let Some((part, rest)) = heading_line(line) {
            let head = line.trim().strip_suffix(rest.as_str()).unwrap_or(line.trim()).trim().trim_end_matches([':', '：']).to_string();
            match heading_languages(&head) {
                langs if langs.is_empty() => ours += 1,
                langs => theirs.extend(langs),
            }
            let in_skills = parts.last().is_some_and(|(p, b)| *p == Part::Skills && !b.iter().all(|l| l.trim().is_empty()));
            if part == Part::Skills && in_skills && rest.is_empty() {
                if let Some(last) = parts.last_mut() {
                    last.1.push(line.trim().to_string());
                }
                continue;
            }
            parts.push((part, if rest.is_empty() { Vec::new() } else { vec![rest] }));
            heading_titles.push(name_case(&head));
        } else if let Some(last) = parts.last_mut() {
            last.1.push(line.clone());
        }
    }
    // The other language that most headings are in, when it is not outnumbered by Norwegian/English ones.
    // On a tie the language listed first wins: the main form (Français) before its regional ones
    // (Français (Canada)), which share most headings.
    let rank = |l: &str| ALL_FOREIGN.iter().position(|(x, _, _)| *x == l).unwrap_or(usize::MAX);
    let count = |l: &str| theirs.iter().filter(|x| **x == l).count();
    let foreign = theirs.iter().copied().max_by(|a, b| count(a).cmp(&count(b)).then(rank(b).cmp(&rank(a)))).filter(|l| count(l) >= ours.max(1));
    let home = p.candidate.city.clone();
    let mut rest = Vec::new();
    let mut leftover = |title: &str, lines: Vec<String>| {
        let body = tidy(&lines.join("\n"));
        if !body.trim().is_empty() {
            rest.push(format!("{}\n{body}", title.to_uppercase()));
        }
    };
    for ((part, body), heading_title) in parts.into_iter().zip(heading_titles) {
        let refs: Vec<&str> = body.iter().map(String::as_str).collect();
        if refs.iter().all(|l| l.trim().is_empty()) {
            continue;
        }
        match part {
            Part::Profile if p.candidate.summary.is_empty() => {
                p.candidate.summary = join_lines(&refs);
                found.push("profil");
            }
            Part::Top => {
                // A paragraph under the contact lines with no heading is the profile.
                let para: Vec<&str> = refs.iter().skip(first_contact + 1).map(|l| l.trim()).filter(|l| !l.is_empty() && !is_contact_line(l)).collect();
                let words: usize = para.iter().map(|l| l.split_whitespace().count()).sum();
                if p.candidate.summary.is_empty() && words >= 25 && para.iter().all(|l| l.split_whitespace().count() >= 4) {
                    p.candidate.summary = join_lines(&para);
                    found.push("profil");
                }
            }
            Part::Skills => {
                let skills = read_skills(&refs);
                if !skills.is_empty() {
                    p.skills.extend(skills);
                    found.push("kompetanse");
                }
            }
            Part::Languages => {
                let langs = read_languages(&refs);
                if !langs.is_empty() {
                    p.languages.extend(langs);
                    found.push("språk");
                }
            }
            Part::Experience => {
                let (jobs, loose) = read_jobs(&refs, &home);
                if !jobs.is_empty() {
                    p.experience.extend(jobs);
                    found.push("jobber");
                }
                if !loose.is_empty() {
                    leftover("Erfaring", loose);
                }
            }
            Part::Education => {
                let (edu, loose) = read_education(&refs);
                if !edu.is_empty() {
                    p.education.extend(edu);
                    found.push("utdanning");
                }
                if !loose.is_empty() {
                    leftover("Utdanning", loose);
                }
            }
            Part::Certifications => {
                let certs = read_certifications(&refs);
                if !certs.is_empty() {
                    p.certifications.extend(certs);
                    found.push("kurs");
                }
            }
            Part::Interests => {
                let interests = read_interests(&refs);
                if !interests.is_empty() {
                    p.interests.extend(interests);
                    found.push("interesser");
                }
            }
            Part::Custom(no, en) => {
                let id = format!("section_imported_{}", p.custom_sections.len() + 1);
                let title = if foreign.is_some() { heading_title.as_str() } else if p.is_english() { en } else { no };
                match read_custom(&refs, &id, title) {
                    Some(section) => {
                        p.custom_sections.push(section);
                        if !found.contains(&"andre seksjoner") {
                            found.push("andre seksjoner");
                        }
                    }
                    None => leftover(title, body),
                }
            }
            Part::Profile => leftover("Profil", body),
            Part::Skip => {}
        }
    }
    // A CV in another language gets that language's headings (and «heute» for a current job).
    if let Some(h) = foreign.and_then(crate::headings::built_in) {
        p.language = crate::model::Language::En;
        for s in &mut p.custom_sections {
            for item in &mut s.items {
                if let Some(d) = item.date.strip_suffix(" – nå") {
                    item.date = format!("{d} – {}", h.present);
                }
            }
        }
        p.options.headings = Some(h);
    } else if looks_english(&text) {
        // An English CV keeps English headings.
        p.language = crate::model::Language::En;
        for s in &mut p.custom_sections {
            for item in &mut s.items {
                if let Some(d) = item.date.strip_suffix(" – nå") {
                    item.date = format!("{d} – Present");
                }
            }
            if let Some((_, en)) = HEADINGS.iter().find_map(|(_, part)| match part {
                Part::Custom(no, en) if *no == s.title => Some((*no, *en)),
                _ => None,
            }) {
                s.title = en.to_string();
            }
        }
    }
    let mut seen = Vec::new();
    found.retain(|f| {
        let new = !seen.contains(f);
        seen.push(*f);
        new
    });
    CvRead { project: p, rest: rest.join("\n\n"), found: found.into_iter().map(String::from).collect() }
}

/// Lines of a paragraph as one text; a word broken with a hyphen at the line end is put together.
fn join_lines(lines: &[&str]) -> String {
    let mut out = String::new();
    for l in lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        if out.ends_with('-') && !out.ends_with(" -") && l.chars().next().is_some_and(char::is_lowercase) {
            out.push_str(l);
        } else {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(l);
        }
    }
    out
}

/// More English than Norwegian words: the CV's headings and dates should be English.
fn looks_english(text: &str) -> bool {
    const EN: &[&str] = &["the", "and", "with", "of", "for", "experience", "education", "skills", "present", "university"];
    const NO: &[&str] = &["og", "med", "for", "av", "erfaring", "utdanning", "kompetanse", "nå", "jeg", "har", "som", "til"];
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).collect();
    let en = words.iter().filter(|w| EN.contains(w)).count();
    let no = words.iter().filter(|w| NO.contains(w)).count();
    en > no * 2 && en >= 5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_marks() {
        for l in ["1 / 2", "Side 1 av 2", "Page 2 of 3", "Ingrid Haugland 1 / 1", "Vebjørn Johannessen 1 / 2", "VebjørnJohannessen1/2"] {
            assert!(is_page_mark(&unspace(l)), "{l}");
        }
        for l in ["Prosjektkoordinator 03.2021 – nå", "Senior Designer 2019 – 2022", "Jobbet 3 av 4 dager"] {
            assert!(!is_page_mark(l), "{l}");
        }
    }
}
