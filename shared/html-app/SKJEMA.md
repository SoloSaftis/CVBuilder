# Fast utfyllingsformat

Bruk nettskjemaet eller `templates/utkast.json`. Ikke endre feltnavn, datatyper
eller `version: 1`. Ikke legg instruksjoner til AI inn i kandidatfeltene.

| Felt | Format og regel |
|---|---|
| language | `no` eller `en`; én ferdig språkversjon per prosjekt |
| candidate | Navn, e-post, telefon, by/land og faglig overskrift kreves ved eksport |
| candidate.summary | Kort profil, helst 30–60 ord; vanlig tekst |
| candidate.photo | Tom streng eller JPEG-data fra byggerens bildeopplasting |
| target | Rolle, arbeidsgiver, eventuell https-lenke, annonsetekst og fagordliste |
| experience.id | Unik ID med bokstaver, tall, understrek eller bindestrek |
| experience.start/end | `ÅÅÅÅ-MM`; `end: null` betyr nåværende jobb. Tom streng betyr uutfylt |
| experience.bullets | Liste med 1–5 tekstpunkter; prioriter 2–3 korte punkter |
| experience.evidence | Forklaring på faktagrunnlaget; ikke en automatisk verifikasjon |
| experience.confirmed | `true` først når kandidaten har bekreftet opplysningene |
| education.start/end | Fire sifre, `ÅÅÅÅ`; sluttår kreves ved eksport |
| skills | Navn, faktagrunnlag og kandidatbekreftelse for hver kompetanse |
| languages | Navn og ærlig nivåbeskrivelse, eksempelvis flytende eller B2 |
| interests | Full bank med ekte interesser; ikke bare dem som skal eksporteres |
| interests.status | `current`, `former` eller `interest` |
| interests.category | `technology`, `outdoors`, `sport`, `creative`, `community`, `other` |
| interests.tags | Liste med konkrete temaord, eksempelvis programmering eller kajakk |
| interests.specificity | Heltall 0–2; hvor konkret formuleringen er, ikke ferdighetsnivå |
| public_context | Presis offentlig opplysning, temaord, https-kilde og faktisk kontrolldato |
| public_context.scope | `employer_activity` eller `professional_bio` |
| public_context.verified | Manuell bekreftelse på at du har lest kilden; ingen nettverifisering i appen |
| options.interest_ids | Høyst tre unike ID-er fra kandidatbekreftede interesser |
| options.page_limit | Fast verdi `2`; ikke øk for å omgå plasskontroll |
| approvals.facts_confirmed | Settes av kandidaten etter gjennomgang; nullstilles ved redigering/import |

Lister skrives som JSON-lister, ikke kommaadskilte tekststrenger. Ukjente felt,
feil typer og for store filer avvises. JSON-kommentarer og avsluttende komma er
ikke tillatt. Prosjektfilen kan være maksimalt 3 MB.

Et utkast skal kunne lagres før alle fakta er klare. Eksport krever riktige
kontaktfelt, minst én jobb eller utdanning, en gyldig tidslinje, kildegrunnlag og
bekreftede påstander. Tomme plassholdere kan ikke brukes som kandidatopplysninger.
Et format som er godkjent, er ikke i seg selv bevis på at en påstand er sann.
