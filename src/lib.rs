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

// ─── RSS groupé format 122 ─────────────────────────────────────────────────
//
// Référence : ATIH « Formats PMSI 2025 » – onglet « RSS groupé format 122 »
// Taille fixe : 192 caractères + (8×nDA) + (8×nDAD) + (29×nZA)
// Source : https://www.atih.sante.fr/formats-pmsi-2025-0

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

    // Prise en charge
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

// ─── RSS groupé format 122 parser ──────────────────────────────────────────

/// Longueur minimale d'une ligne : 192 caractères de tête fixe
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

    let nb_da: usize = f(134, 2).parse().unwrap_or(0);
    let nb_dad: usize = f(136, 2).parse().unwrap_or(0);
    let nb_za: usize = f(138, 3).parse().unwrap_or(0);

    let expected_len = FIXED_LEN + nb_da * 8 + nb_dad * 8 + nb_za * 29;
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

    let mut actes = Vec::with_capacity(nb_za);
    for _ in 0..nb_za {
        let z = &chars[pos..pos + 29];
        let fz = |start: usize, len: usize| -> String {
            z[start..start + len]
                .iter()
                .collect::<String>()
                .trim()
                .to_string()
        };
        actes.push(ActeCcam {
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
        });
        pos += 29;
    }

    Ok(Rss {
        ligne: line_num,
        version_classification: f(1, 2),
        cmd: f(3, 2),
        ghm: f(5, 4),
        version_format_rss: f(10, 3),
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
        conversion_hc: f(178, 1),
        raac: f(179, 1),
        contexte_patient: f(180, 1),
        admin_produit_rh: f(181, 1),
        rescrit_tarifaire: f(182, 1),
        cat_nb_interventions: f(183, 1),
        non_programme: f(184, 1),
        passage_urgences: f(185, 1),
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

    /// Construit une ligne RSS groupé format 122 valide pour les tests.
    fn make_rss_line(nb_da: usize, nb_dad: usize, nb_za: usize) -> String {
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
        line[9] = '1';
        line[10] = '2';
        line[11] = '2';
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
        // Zones d'actes (29 chars)
        for _ in 0..nb_za {
            result.push_str("01012024"); // date (8)
            result.push_str("ZBQK002"); // code CCAM (7)
            result.push_str("   "); // extension PMSI (3)
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
}
