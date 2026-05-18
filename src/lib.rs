// Copyright © 2025 William Gacquer — tous droits réservés

use anyhow::{Context, Result, anyhow, bail};
use encoding_rs::WINDOWS_1252;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

// ─── Public API ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ConversionResult {
    pub input: String,
    pub files: Vec<FileResult>,
}

#[derive(Debug, Serialize)]
pub struct FileResult {
    pub path: String,
    pub encoding: String,
    pub record_count: usize,
    pub records: Vec<Rss>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ParseError>,
}

#[derive(Debug, Serialize)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

// ─── RSS groupé — formats 116 à 122 ─────────────────────────────────────
//
// Référence : ATIH « Formats PMSI MCO »
//   format 116 → 2013-2015  (document du 19/12/2012)
//   format 117 → 2016       https://www.atih.sante.fr/node/2833
//   format 118 → 2017-2018  https://www.atih.sante.fr/node/3050
//   format 119 → 2019       https://www.atih.sante.fr/formats-pmsi-2019
//   format 120 → 2020-2021  https://www.atih.sante.fr/formats-pmsi-2021
//   format 121 → 2022       https://www.atih.sante.fr/formats-pmsi-2022
//   format 122 → 2023-2025  https://www.atih.sante.fr/formats-pmsi-2025-0
//
// Taille fixe (en-tête) : 192 caractères pour tous les formats supportés.
// Zone d'acte CCAM (ZA) : 26 chars en format 116 (sans extension_pmsi) ;
//                          29 chars en formats 117-122.
// Partie variable        : (8×nDA) + (8×nDAD) + (za_len×nZA)
//
// Évolution des champs en fin d'en-tête (positions 163-192) :
//   fmt 116 : 163-177=innovation, 178-192=zone_réservée(15)
//   fmt 117 : 163-177=innovation, 178-179=nb_ivg, 180-183=annee_ivg,
//             184-187=filler, 188-189=naissances_vivantes, 190-192=zone_rés.
//   fmt 118 : 163-177=innovation, 178-189=filler(12), 190-192=zone_réservée
//   fmt 119 : 163-177=innovation, 178=conv_hc, 179=raac, 180-189=filler
//   fmt 120 : …119… + 180=contexte_patient, 181=admin_produit_rh,
//             182=rescrit_tarifaire, 183=cat_nb_interventions, 184-189=filler
//   fmt 121 : …120… + 184=non_programme, 185-189=filler
//   fmt 122 : …121… + 185=passage_urgences, 186-189=filler

#[derive(Debug, Serialize)]
pub struct Rss {
    pub ligne: usize,

    // Groupage
    pub version_classification: String, // 1-2
    pub cmd: String,                    // 3-4
    pub ghm: String,                    // 5-8
    pub version_format_rss: String,     // 10-12  (valeur fixe = "122")
    pub code_retour_groupage: String,   // 13-15

    // Identification
    pub finess: String,             // 16-24
    pub version_format_rum: String, // 25-27  (valeur fixe = "022")
    pub num_rss: String,            // 28-47
    pub num_sejour: String,         // 48-67
    pub num_rum: String,            // 68-77

    // Patient
    pub date_naissance: String, // 78-85  (JJMMAAAA)
    pub sexe: String,           // 86     (1=H, 2=F, 3=indéterminé)
    pub num_um: String,         // 87-90
    #[serde(skip_serializing_if = "String::is_empty")]
    pub type_autorisation_lit: String, // 91-92

    // Entrée dans l'UM
    pub date_entree: String, // 93-100 (JJMMAAAA)
    pub mode_entree: String, // 101
    #[serde(skip_serializing_if = "String::is_empty")]
    pub provenance: String, // 102

    // Sortie de l'UM
    pub date_sortie: String, // 103-110 (JJMMAAAA)
    pub mode_sortie: String, // 111
    #[serde(skip_serializing_if = "String::is_empty")]
    pub destination: String, // 112

    // Divers
    pub code_postal: String, // 113-117
    #[serde(skip_serializing_if = "String::is_empty")]
    pub poids_naissance: String, // 118-121 (grammes)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub age_gestationnel: String, // 122-123 (semaines)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub date_ddr: String, // 124-131 (JJMMAAAA)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub nb_seances: String, // 132-133

    // Compteurs (lus en clair pour calculer la partie variable)
    pub nb_da: usize,  // 134-135
    pub nb_dad: usize, // 136-137
    pub nb_za: usize,  // 138-140

    // Diagnostics principaux
    pub dp: String, // 141-148 (CIM-10)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub dr: String, // 149-156 (CIM-10)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub igs2: String, // 157-159
    #[serde(skip_serializing_if = "String::is_empty")]
    pub confirmation_codage: String, // 160

    // Radiothérapie
    #[serde(skip_serializing_if = "String::is_empty")]
    pub type_machine_rt: String, // 161
    #[serde(skip_serializing_if = "String::is_empty")]
    pub type_dosimetrie: String, // 162
    #[serde(skip_serializing_if = "String::is_empty")]
    pub num_innovation: String, // 163-177

    // Obstétrique — format 117 uniquement
    #[serde(skip_serializing_if = "String::is_empty")]
    pub nb_ivg_anterieures: String, // 178-179
    #[serde(skip_serializing_if = "String::is_empty")]
    pub annee_ivg_precedente: String, // 180-183
    #[serde(skip_serializing_if = "String::is_empty")]
    pub nb_naissances_vivantes: String, // 188-189

    // Prise en charge — formats 119+
    #[serde(skip_serializing_if = "String::is_empty")]
    pub conversion_hc: String, // 178
    #[serde(skip_serializing_if = "String::is_empty")]
    pub raac: String, // 179
    #[serde(skip_serializing_if = "String::is_empty")]
    pub contexte_patient: String, // 180
    #[serde(skip_serializing_if = "String::is_empty")]
    pub admin_produit_rh: String, // 181
    #[serde(skip_serializing_if = "String::is_empty")]
    pub rescrit_tarifaire: String, // 182
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cat_nb_interventions: String, // 183
    #[serde(skip_serializing_if = "String::is_empty")]
    pub non_programme: String, // 184
    #[serde(skip_serializing_if = "String::is_empty")]
    pub passage_urgences: String, // 185

    // Partie variable
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics_associes: Vec<String>, // nDA × 8 chars (CIM-10)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics_documentaires: Vec<String>, // nDAD × 8 chars (CIM-10)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actes: Vec<ActeCcam>, // nZA × 29 chars
}

/// Zone d'acte CCAM (29 caractères dans le RSS groupé)
#[derive(Debug, Serialize)]
pub struct ActeCcam {
    pub date_realisation: String, // 8  (JJMMAAAA)
    pub code_ccam: String,        // 7
    #[serde(skip_serializing_if = "String::is_empty")]
    pub extension_pmsi: String, // 3
    pub phase: String,            // 1
    pub activite: String,         // 1
    #[serde(skip_serializing_if = "String::is_empty")]
    pub extension_documentaire: String, // 1
    #[serde(skip_serializing_if = "String::is_empty")]
    pub modificateurs: String, // 4
    #[serde(skip_serializing_if = "String::is_empty")]
    pub remboursement_exceptionnel: String, // 1
    #[serde(skip_serializing_if = "String::is_empty")]
    pub association_non_prevue: String, // 1
    pub nb_realisations: String,  // 2
}

// ─── I/O ───────────────────────────────────────────────────────────────────

pub fn convert_path(input: &Path) -> Result<ConversionResult> {
    let files = collect_grp_files(input)?;
    let parsed_files = files
        .iter()
        .map(|path| parse_file(path))
        .collect::<Result<Vec<_>>>()?;

    Ok(ConversionResult {
        input: input.display().to_string(),
        files: parsed_files,
    })
}

pub fn write_json(result: &ConversionResult, output: Option<&Path>, pretty: bool) -> Result<()> {
    let json = if pretty {
        serde_json::to_string_pretty(result)?
    } else {
        serde_json::to_string(result)?
    };

    if let Some(path) = output {
        fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    } else {
        println!("{json}");
    }

    Ok(())
}

/// Écrit un objet JSON compact par enregistrement RSS, un par ligne (format NDJSON).
pub fn write_json_lines(result: &ConversionResult, output: Option<&Path>) -> Result<()> {
    let mut out = String::new();
    for file in &result.files {
        for record in &file.records {
            out.push_str(&serde_json::to_string(record)?);
            out.push('\n');
        }
    }

    if let Some(path) = output {
        fs::write(path, &out).with_context(|| format!("failed to write {}", path.display()))?;
    } else {
        print!("{out}");
    }

    Ok(())
}

// ─── File collection ───────────────────────────────────────────────────────

fn collect_grp_files(input: &Path) -> Result<Vec<PathBuf>> {
    if input.is_file() {
        return Ok(vec![input.to_path_buf()]);
    }

    if !input.is_dir() {
        bail!("{} is neither a file nor a directory", input.display());
    }

    let mut files = fs::read_dir(input)
        .with_context(|| format!("failed to read {}", input.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && has_grp_extension(path))
        .collect::<Vec<_>>();

    files.sort();

    if files.is_empty() {
        bail!("no .grp files found in {}", input.display());
    }

    Ok(files)
}

fn has_grp_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("grp"))
}

// ─── File parsing ──────────────────────────────────────────────────────────

fn parse_file(path: &Path) -> Result<FileResult> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;

    let (text, encoding) = decode_bytes(&bytes);

    let mut records = Vec::new();
    let mut errors = Vec::new();

    for (line_idx, raw_line) in text.lines().enumerate() {
        let line_num = line_idx + 1;
        let line = raw_line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }

        let chars: Vec<char> = line.chars().collect();
        match parse_rss(&chars, line_num) {
            Ok(rss) => records.push(rss),
            Err(e) => errors.push(ParseError {
                line: line_num,
                message: e.to_string(),
            }),
        }
    }

    if records.is_empty() {
        return Err(anyhow!(
            "{}: aucun enregistrement valide ({} erreur(s))",
            path.display(),
            errors.len()
        ));
    }

    let record_count = records.len();
    Ok(FileResult {
        path: path.display().to_string(),
        encoding: encoding.to_string(),
        record_count,
        records,
        errors,
    })
}

// ─── Parser multi-format ───────────────────────────────────────────────────

/// Champs dont la présence et la signification varient selon la version du format.
#[derive(Default)]
struct VersionFields {
    // Format 117 uniquement
    nb_ivg_anterieures: String,
    annee_ivg_precedente: String,
    nb_naissances_vivantes: String,
    // Formats 119+
    conversion_hc: String,
    raac: String,
    // Formats 120+
    contexte_patient: String,
    admin_produit_rh: String,
    rescrit_tarifaire: String,
    cat_nb_interventions: String,
    // Formats 121+
    non_programme: String,
    // Formats 122+
    passage_urgences: String,
}

/// Longueur fixe de l'en-tête, identique pour tous les formats.
const FIXED_LEN: usize = 192;

fn parse_rss(chars: &[char], line_num: usize) -> Result<Rss> {
    if chars.len() < FIXED_LEN {
        bail!(
            "ligne {} trop courte : {} caractères (minimum {})",
            line_num,
            chars.len(),
            FIXED_LEN
        );
    }

    // Extraction d'un champ : position 1-based, longueur en caractères
    let f = |pos: usize, len: usize| -> String {
        let start = pos - 1;
        let end = (start + len).min(chars.len());
        chars[start..end]
            .iter()
            .collect::<String>()
            .trim()
            .to_string()
    };

    // La version du format est à la position 10-12 de chaque ligne.
    // Elle détermine la taille des ZA et les champs spécifiques en 178-189.
    let version_format_rss = f(10, 3);
    let (za_len, vf): (usize, VersionFields) = match version_format_rss.as_str() {
        "116" | "118" => (
            if version_format_rss == "116" { 26 } else { 29 },
            VersionFields::default(),
        ),
        "117" => (
            29,
            VersionFields {
                nb_ivg_anterieures: f(178, 2),
                annee_ivg_precedente: f(180, 4),
                nb_naissances_vivantes: f(188, 2),
                ..Default::default()
            },
        ),
        "119" => (
            29,
            VersionFields {
                conversion_hc: f(178, 1),
                raac: f(179, 1),
                ..Default::default()
            },
        ),
        "120" => (
            29,
            VersionFields {
                conversion_hc: f(178, 1),
                raac: f(179, 1),
                contexte_patient: f(180, 1),
                admin_produit_rh: f(181, 1),
                rescrit_tarifaire: f(182, 1),
                cat_nb_interventions: f(183, 1),
                ..Default::default()
            },
        ),
        "121" => (
            29,
            VersionFields {
                conversion_hc: f(178, 1),
                raac: f(179, 1),
                contexte_patient: f(180, 1),
                admin_produit_rh: f(181, 1),
                rescrit_tarifaire: f(182, 1),
                cat_nb_interventions: f(183, 1),
                non_programme: f(184, 1),
                ..Default::default()
            },
        ),
        "122" => (
            29,
            VersionFields {
                conversion_hc: f(178, 1),
                raac: f(179, 1),
                contexte_patient: f(180, 1),
                admin_produit_rh: f(181, 1),
                rescrit_tarifaire: f(182, 1),
                cat_nb_interventions: f(183, 1),
                non_programme: f(184, 1),
                passage_urgences: f(185, 1),
                ..Default::default()
            },
        ),
        other => bail!(
            "ligne {} : version_format_rss inconnue « {} » \
             (formats supportés : 116 à 122 ; les formats antérieurs à 2013 \
             ne sont pas encore pris en charge)",
            line_num,
            other
        ),
    };

    let nb_da: usize = f(134, 2).parse().unwrap_or(0);
    let nb_dad: usize = f(136, 2).parse().unwrap_or(0);
    let nb_za: usize = f(138, 3).parse().unwrap_or(0);

    let expected_len = FIXED_LEN + nb_da * 8 + nb_dad * 8 + nb_za * za_len;
    if chars.len() < expected_len {
        bail!(
            "ligne {} : trop courte pour la partie variable ({} < {}; nDA={}, nDAD={}, nZA={})",
            line_num,
            chars.len(),
            expected_len,
            nb_da,
            nb_dad,
            nb_za
        );
    }

    // Lecture de la partie variable
    let mut pos = FIXED_LEN;

    let mut diagnostics_associes = Vec::with_capacity(nb_da);
    for _ in 0..nb_da {
        let code: String = chars[pos..pos + 8]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        diagnostics_associes.push(code);
        pos += 8;
    }

    let mut diagnostics_documentaires = Vec::with_capacity(nb_dad);
    for _ in 0..nb_dad {
        let code: String = chars[pos..pos + 8]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        diagnostics_documentaires.push(code);
        pos += 8;
    }

    // Zone d'acte CCAM : 26 chars en format 116 (sans extension_pmsi),
    //                     29 chars en formats 117-122.
    let mut actes = Vec::with_capacity(nb_za);
    for _ in 0..nb_za {
        let z = &chars[pos..pos + za_len];
        let fz = |start: usize, len: usize| -> String {
            z[start..start + len]
                .iter()
                .collect::<String>()
                .trim()
                .to_string()
        };
        actes.push(if za_len == 26 {
            ActeCcam {
                date_realisation: fz(0, 8),
                code_ccam: fz(8, 7),
                extension_pmsi: String::new(),
                phase: fz(15, 1),
                activite: fz(16, 1),
                extension_documentaire: fz(17, 1),
                modificateurs: fz(18, 4),
                remboursement_exceptionnel: fz(22, 1),
                association_non_prevue: fz(23, 1),
                nb_realisations: fz(24, 2),
            }
        } else {
            ActeCcam {
                date_realisation: fz(0, 8),
                code_ccam: fz(8, 7),
                extension_pmsi: fz(15, 3),
                phase: fz(18, 1),
                activite: fz(19, 1),
                extension_documentaire: fz(20, 1),
                modificateurs: fz(21, 4),
                remboursement_exceptionnel: fz(25, 1),
                association_non_prevue: fz(26, 1),
                nb_realisations: fz(27, 2),
            }
        });
        pos += za_len;
    }

    Ok(Rss {
        ligne: line_num,
        version_classification: f(1, 2),
        cmd: f(3, 2),
        ghm: f(5, 4),
        version_format_rss,
        code_retour_groupage: f(13, 3),
        finess: f(16, 9),
        version_format_rum: f(25, 3),
        num_rss: f(28, 20),
        num_sejour: f(48, 20),
        num_rum: f(68, 10),
        date_naissance: f(78, 8),
        sexe: f(86, 1),
        num_um: f(87, 4),
        type_autorisation_lit: f(91, 2),
        date_entree: f(93, 8),
        mode_entree: f(101, 1),
        provenance: f(102, 1),
        date_sortie: f(103, 8),
        mode_sortie: f(111, 1),
        destination: f(112, 1),
        code_postal: f(113, 5),
        poids_naissance: f(118, 4),
        age_gestationnel: f(122, 2),
        date_ddr: f(124, 8),
        nb_seances: f(132, 2),
        nb_da,
        nb_dad,
        nb_za,
        dp: f(141, 8),
        dr: f(149, 8),
        igs2: f(157, 3),
        confirmation_codage: f(160, 1),
        type_machine_rt: f(161, 1),
        type_dosimetrie: f(162, 1),
        num_innovation: f(163, 15),
        nb_ivg_anterieures: vf.nb_ivg_anterieures,
        annee_ivg_precedente: vf.annee_ivg_precedente,
        nb_naissances_vivantes: vf.nb_naissances_vivantes,
        conversion_hc: vf.conversion_hc,
        raac: vf.raac,
        contexte_patient: vf.contexte_patient,
        admin_produit_rh: vf.admin_produit_rh,
        rescrit_tarifaire: vf.rescrit_tarifaire,
        cat_nb_interventions: vf.cat_nb_interventions,
        non_programme: vf.non_programme,
        passage_urgences: vf.passage_urgences,
        diagnostics_associes,
        diagnostics_documentaires,
        actes,
    })
}

// ─── Décodage ──────────────────────────────────────────────────────────────

fn decode_bytes(bytes: &[u8]) -> (String, &'static str) {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return (text.to_string(), "utf-8");
    }
    let (text, _, _) = WINDOWS_1252.decode(bytes);
    (text.into_owned(), "windows-1252")
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Construit une ligne RSS groupé valide pour les tests.
    /// `fmt` : version du format ("120", "121" ou "122")
    /// `non_prog` / `passage_urg` : valeur à écrire aux positions 184 / 185
    fn make_rss_line_fmt(
        fmt: &str,
        nb_da: usize,
        nb_dad: usize,
        nb_za: usize,
        non_prog: char,
        passage_urg: char,
    ) -> String {
        let mut line = vec![' '; FIXED_LEN];

        // version_classification (1-2)
        line[0] = '1';
        line[1] = '1';
        // cmd (3-4)
        line[2] = '0';
        line[3] = '1';
        // ghm (5-8)
        line[4] = '0';
        line[5] = '1';
        line[6] = 'M';
        line[7] = '1';
        // version_format_rss (10-12)
        let fmt_bytes: Vec<char> = fmt.chars().collect();
        line[9] = fmt_bytes[0];
        line[10] = fmt_bytes[1];
        line[11] = fmt_bytes[2];
        // non_programme (184) et passage_urgences (185)
        line[183] = non_prog;
        line[184] = passage_urg;
        // code_retour_groupage (13-15)
        line[12] = '0';
        line[13] = '0';
        line[14] = '0';
        // finess (16-24)
        for (i, ch) in "010000000".chars().enumerate() {
            line[15 + i] = ch;
        }
        // version_format_rum (25-27)
        line[24] = '0';
        line[25] = '2';
        line[26] = '2';
        // date_naissance (78-85)
        for (i, ch) in "01011990".chars().enumerate() {
            line[77 + i] = ch;
        }
        // sexe (86)
        line[85] = '1';
        // num_um (87-90)
        for (i, ch) in "0001".chars().enumerate() {
            line[86 + i] = ch;
        }
        // date_entree (93-100)
        for (i, ch) in "01012024".chars().enumerate() {
            line[92 + i] = ch;
        }
        // mode_entree (101)
        line[100] = '8';
        // date_sortie (103-110)
        for (i, ch) in "05012024".chars().enumerate() {
            line[102 + i] = ch;
        }
        // mode_sortie (111)
        line[110] = '8';
        // code_postal (113-117)
        for (i, ch) in "75001".chars().enumerate() {
            line[112 + i] = ch;
        }
        // nb_seances (132-133)
        line[131] = '0';
        line[132] = '0';
        // nb_da (134-135)
        let nda = format!("{:02}", nb_da);
        line[133] = nda.chars().nth(0).unwrap();
        line[134] = nda.chars().nth(1).unwrap();
        // nb_dad (136-137)
        let ndad = format!("{:02}", nb_dad);
        line[135] = ndad.chars().nth(0).unwrap();
        line[136] = ndad.chars().nth(1).unwrap();
        // nb_za (138-140)
        let nza = format!("{:03}", nb_za);
        line[137] = nza.chars().nth(0).unwrap();
        line[138] = nza.chars().nth(1).unwrap();
        line[139] = nza.chars().nth(2).unwrap();
        // dp (141-148)
        for (i, ch) in "S72.00  ".chars().enumerate() {
            line[140 + i] = ch;
        }

        let mut result: String = line.into_iter().collect();

        // DA
        for i in 0..nb_da {
            result.push_str(&format!("Z96.64{:02}", i));
        }
        // DAD
        for i in 0..nb_dad {
            result.push_str(&format!("Z96.65{:02}", i));
        }
        // Zones d'actes : 26 chars en format 116, 29 chars sinon
        for _ in 0..nb_za {
            result.push_str("01012024"); // date (8)
            result.push_str("ZBQK002"); // code CCAM (7)
            if fmt != "116" {
                result.push_str("   "); // extension PMSI (3) — absent en format 116
            }
            result.push('0'); // phase (1)
            result.push('1'); // activite (1)
            result.push(' '); // ext documentaire (1)
            result.push_str("    "); // modificateurs (4)
            result.push(' '); // remboursement exceptionnel (1)
            result.push(' '); // association non prévue (1)
            result.push_str("01"); // nb réalisations (2)
        }

        result
    }

    fn make_rss_line(nb_da: usize, nb_dad: usize, nb_za: usize) -> String {
        make_rss_line_fmt("122", nb_da, nb_dad, nb_za, '1', '5')
    }

    #[test]
    fn parses_rss_sans_partie_variable() {
        let line = make_rss_line(0, 0, 0);
        let chars: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars, 1).expect("parse RSS");

        assert_eq!(rss.finess, "010000000");
        assert_eq!(rss.cmd, "01");
        assert_eq!(rss.ghm, "01M1");
        assert_eq!(rss.dp, "S72.00");
        assert_eq!(rss.nb_da, 0);
        assert_eq!(rss.nb_dad, 0);
        assert_eq!(rss.nb_za, 0);
        assert!(rss.actes.is_empty());
    }

    #[test]
    fn parses_rss_avec_actes_ccam() {
        let line = make_rss_line(1, 0, 2);
        let chars: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars, 1).expect("parse RSS");

        assert_eq!(rss.nb_da, 1);
        assert_eq!(rss.nb_za, 2);
        assert_eq!(rss.diagnostics_associes.len(), 1);
        assert_eq!(rss.actes.len(), 2);
        assert_eq!(rss.actes[0].code_ccam, "ZBQK002");
        assert_eq!(rss.actes[0].activite, "1");
        assert_eq!(rss.actes[0].nb_realisations, "01");
        assert_eq!(rss.actes[0].date_realisation, "01012024");
    }

    #[test]
    fn rejette_ligne_trop_courte() {
        let chars: Vec<char> = "trop court".chars().collect();
        assert!(parse_rss(&chars, 1).is_err());
    }

    #[test]
    fn collecte_uniquement_les_fichiers_grp() {
        let dir = tempdir().expect("tempdir");
        let grp_path = dir.path().join("sample.grp");
        let txt_path = dir.path().join("sample.txt");

        let content = format!("{}\n", make_rss_line(0, 0, 0));
        fs::write(&grp_path, &content).expect("write grp");
        fs::write(&txt_path, "ignore me").expect("write txt");

        let result = convert_path(dir.path()).expect("convert directory");
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, grp_path.display().to_string());
        assert_eq!(result.files[0].record_count, 1);
    }

    // ── Tests multi-format ──────────────────────────────────────────────────

    #[test]
    fn format_120_sans_non_programme_ni_passage_urgences() {
        // Format 120 (2021) : positions 184-185 sont du filler, non_programme et
        // passage_urgences doivent être absents du résultat (chaînes vides).
        let line = make_rss_line_fmt("120", 0, 0, 0, '1', '5');
        let chars: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars, 1).expect("parse format 120");

        assert_eq!(rss.version_format_rss, "120");
        assert!(
            rss.non_programme.is_empty(),
            "non_programme doit être vide en format 120"
        );
        assert!(
            rss.passage_urgences.is_empty(),
            "passage_urgences doit être vide en format 120"
        );
    }

    #[test]
    fn format_121_avec_non_programme_sans_passage_urgences() {
        // Format 121 (2022) : position 184 = non_programme, 185 = filler.
        let line = make_rss_line_fmt("121", 0, 0, 0, '1', ' ');
        let chars: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars, 1).expect("parse format 121");

        assert_eq!(rss.version_format_rss, "121");
        assert_eq!(
            rss.non_programme, "1",
            "non_programme doit être '1' en format 121"
        );
        assert!(
            rss.passage_urgences.is_empty(),
            "passage_urgences doit être vide en format 121"
        );
    }

    #[test]
    fn format_122_avec_non_programme_et_passage_urgences() {
        // Format 122 (2023-2025) : 184 = non_programme, 185 = passage_urgences.
        let line = make_rss_line_fmt("122", 0, 0, 0, '1', '5');
        let chars: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars, 1).expect("parse format 122");

        assert_eq!(rss.version_format_rss, "122");
        assert_eq!(rss.non_programme, "1");
        assert_eq!(rss.passage_urgences, "5");
    }

    #[test]
    fn rejette_version_format_inconnue() {
        // Formats > 122 (trop récent) ou < 116 (trop ancien) sont rejetés.
        for bad_fmt in ["123", "115", "099"] {
            let line = make_rss_line_fmt(bad_fmt, 0, 0, 0, ' ', ' ');
            let chars: Vec<char> = line.chars().collect();
            let err = parse_rss(&chars, 1).unwrap_err();
            assert!(
                err.to_string().contains("version_format_rss inconnue"),
                "message d'erreur inattendu pour fmt={bad_fmt}: {err}"
            );
        }
    }

    // ── Tests formats 116-119 ───────────────────────────────────────────────

    #[test]
    fn format_116_parse_sans_erreur_champs_vides() {
        // Format 116 (2013-2015) : pos 178-192 = zone_réservée, tous les champs
        // spécifiques aux formats postérieurs doivent être vides.
        let line = make_rss_line_fmt("116", 0, 0, 0, ' ', ' ');
        let chars: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars, 1).expect("parse format 116");

        assert_eq!(rss.version_format_rss, "116");
        assert!(rss.nb_ivg_anterieures.is_empty());
        assert!(rss.annee_ivg_precedente.is_empty());
        assert!(rss.nb_naissances_vivantes.is_empty());
        assert!(rss.conversion_hc.is_empty());
        assert!(rss.raac.is_empty());
        assert!(rss.non_programme.is_empty());
        assert!(rss.passage_urgences.is_empty());
    }

    #[test]
    fn format_116_acte_ccam_26_chars() {
        // Format 116 : la ZA fait 26 chars (pas d'extension_pmsi).
        // L'acte doit être parsé correctement, extension_pmsi reste vide.
        let line = make_rss_line_fmt("116", 0, 0, 1, ' ', ' ');
        let chars: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars, 1).expect("parse format 116 avec ZA");

        assert_eq!(rss.actes.len(), 1);
        assert_eq!(rss.actes[0].code_ccam, "ZBQK002");
        assert_eq!(rss.actes[0].activite, "1");
        assert_eq!(rss.actes[0].nb_realisations, "01");
        assert!(rss.actes[0].extension_pmsi.is_empty(), "extension_pmsi absent en fmt 116");
    }

    #[test]
    fn format_117_champs_ivg() {
        // Format 117 (2016) : nb_ivg et annee_ivg aux positions 178-183.
        // On construit une ligne avec des valeurs fictives à ces positions.
        let mut line = make_rss_line_fmt("117", 0, 0, 0, ' ', ' ');
        // Positions 178-179 (0-indexed: 177-178) = nb_ivg_anterieures
        let mut chars: Vec<char> = line.chars().collect();
        chars[177] = '0';
        chars[178] = '2'; // nb_ivg = "02"
        // Positions 180-183 (0-indexed: 179-182) = annee_ivg_precedente
        chars[179] = '2';
        chars[180] = '0';
        chars[181] = '1';
        chars[182] = '5'; // annee = "2015"
        // Positions 188-189 (0-indexed: 187-188) = nb_naissances_vivantes
        chars[187] = '0';
        chars[188] = '3'; // naissances = "03"
        line = chars.into_iter().collect();

        let chars2: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars2, 1).expect("parse format 117");

        assert_eq!(rss.version_format_rss, "117");
        assert_eq!(rss.nb_ivg_anterieures, "02");
        assert_eq!(rss.annee_ivg_precedente, "2015");
        assert_eq!(rss.nb_naissances_vivantes, "03");
        assert!(rss.conversion_hc.is_empty(), "conversion_hc absent en fmt 117");
        assert!(rss.raac.is_empty(), "raac absent en fmt 117");
    }

    #[test]
    fn format_118_parse_sans_champs_specifiques() {
        // Format 118 (2017-2018) : pos 178-189 = filler, tous vides.
        let line = make_rss_line_fmt("118", 0, 0, 0, ' ', ' ');
        let chars: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars, 1).expect("parse format 118");

        assert_eq!(rss.version_format_rss, "118");
        assert!(rss.nb_ivg_anterieures.is_empty());
        assert!(rss.conversion_hc.is_empty());
        assert!(rss.non_programme.is_empty());
    }

    #[test]
    fn format_119_conversion_hc_et_raac() {
        // Format 119 (2019) : pos 178 = conversion_hc, 179 = raac.
        let mut line = make_rss_line_fmt("119", 0, 0, 0, ' ', ' ');
        let mut chars: Vec<char> = line.chars().collect();
        chars[177] = '1'; // conversion_hc = '1'
        chars[178] = '1'; // raac = '1'
        line = chars.into_iter().collect();

        let chars2: Vec<char> = line.chars().collect();
        let rss = parse_rss(&chars2, 1).expect("parse format 119");

        assert_eq!(rss.version_format_rss, "119");
        assert_eq!(rss.conversion_hc, "1");
        assert_eq!(rss.raac, "1");
        assert!(rss.contexte_patient.is_empty(), "contexte_patient absent en fmt 119");
        assert!(rss.non_programme.is_empty());
    }

    #[test]
    fn fichier_multi_format_trois_versions() {
        // Un fichier contenant des lignes de trois formats différents (120, 121, 122)
        // doit parser correctement chaque ligne selon sa version.
        let dir = tempdir().expect("tempdir");
        let grp_path = dir.path().join("mix.grp");

        let line120 = make_rss_line_fmt("120", 0, 0, 0, ' ', ' ');
        let line121 = make_rss_line_fmt("121", 0, 0, 0, '1', ' ');
        let line122 = make_rss_line_fmt("122", 0, 0, 0, '1', '5');
        let content = format!("{line120}\n{line121}\n{line122}\n");
        fs::write(&grp_path, &content).expect("write grp");

        let result = convert_path(&grp_path).expect("convert multi-format");
        assert_eq!(result.files[0].record_count, 3);
        assert_eq!(result.files[0].errors.len(), 0);

        let r0 = &result.files[0].records[0];
        let r1 = &result.files[0].records[1];
        let r2 = &result.files[0].records[2];

        assert_eq!(r0.version_format_rss, "120");
        assert!(r0.non_programme.is_empty());
        assert!(r0.passage_urgences.is_empty());

        assert_eq!(r1.version_format_rss, "121");
        assert_eq!(r1.non_programme, "1");
        assert!(r1.passage_urgences.is_empty());

        assert_eq!(r2.version_format_rss, "122");
        assert_eq!(r2.non_programme, "1");
        assert_eq!(r2.passage_urgences, "5");
    }

    #[test]
    fn write_json_lines_produit_une_ligne_par_rss() {
        let dir = tempdir().expect("tempdir");
        let grp_path = dir.path().join("test.grp");
        let out_path = dir.path().join("out.ndjson");

        let line1 = make_rss_line_fmt("122", 0, 0, 0, '1', '5');
        let line2 = make_rss_line_fmt("122", 0, 0, 0, '0', '3');
        fs::write(&grp_path, format!("{line1}\n{line2}\n")).expect("write grp");

        let result = convert_path(&grp_path).expect("convert");
        write_json_lines(&result, Some(&out_path)).expect("write lines");

        let content = fs::read_to_string(&out_path).expect("read output");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "doit produire exactement 2 lignes");

        // chaque ligne est du JSON valide dont la clef version_format_rss vaut "122"
        for line in &lines {
            let v: serde_json::Value = serde_json::from_str(line)
                .expect("chaque ligne doit être du JSON valide");
            assert_eq!(v["version_format_rss"], "122");
        }
    }
}
