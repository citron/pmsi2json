# pmsi2json

> **Convertisseur de fichiers PMSI RSS groupé (`.grp`) vers JSON**

[![Licence EUPL 1.2](https://img.shields.io/badge/licence-EUPL%201.2-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)

---

## À propos

**pmsi2json** est un outil en ligne de commande écrit en [Rust](https://www.rust-lang.org/) qui
lit les fichiers RSS groupés PMSI MCO (extension `.grp`) produits par les logiciels de groupage
ATIH et les convertit en JSON structuré.

Il a été développé par **William Gacquer** (CHU Amiens) pour faciliter l'exploitation des données
PMSI dans des pipelines de données modernes (Python, R, SQL, BI…).

---

## Licence

Ce code est publié sous licence **[EUPL 1.2](LICENSE)** (European Union Public Licence).
William Gacquer en est l'auteur et le met librement à disposition de quiconque souhaite l'utiliser,
le modifier ou le redistribuer, dans le respect des termes de la licence.

---

## À quoi ça sert ?

Les fichiers `.grp` sont des **RSS groupés** (Résumés de Sortie Standardisés groupés) transmis
chaque mois à l'ATIH. Chaque ligne décrit un séjour hospitalier MCO : diagnostic principal,
diagnostics associés, actes CCAM réalisés, durée de séjour, GHM, etc.

Ces fichiers sont en format **texte à largeur fixe**, défini par les spécifications annuelles
[Formats PMSI MCO](https://www.atih.sante.fr/formats-pmsi-2025-0) publiées par l'ATIH.
Ils sont **difficiles à exploiter directement** sans connaître précisément chaque position de champ.

**pmsi2json** détecte automatiquement la version du format (encodée dans chaque ligne du fichier),
extrait tous les champs et produit un JSON propre, ce qui permet de :

- Analyser les données PMSI avec Python / pandas, R, DuckDB, etc.
- Alimenter une base de données ou un entrepôt de données
- Effectuer des contrôles qualité sur les RSS
- Développer des outils de reporting ou de facturation

---

## Versions de format supportées

Chaque ligne d'un fichier `.grp` contient sa propre version de format (positions 10-12). Le
programme gère nativement les fichiers multi-format (lignes de versions différentes dans un même
fichier).

| Format | Années d'usage | Statut        |
|--------|----------------|---------------|
| 122    | 2023 – 2025    | ✅ Supporté   |
| 121    | 2022           | ✅ Supporté   |
| 120    | 2020 – 2021    | ✅ Supporté   |
| 119    | 2019           | ✅ Supporté   |
| 118    | 2017 – 2018    | ✅ Supporté   |
| 117    | 2016           | ✅ Supporté   |
| 116    | 2013 – 2015    | ✅ Supporté   |
| ≤ 115  | ≤ 2012         | ❌ Non supporté (spécifications non disponibles) |

> **Note :** Les formats antérieurs à 116 (avant 2013) ne sont pas encore pris en charge,
> faute de documentation officielle accessible. Un message d'erreur explicite est retourné
> pour ces lignes.

---

## Champs extraits

Pour chaque séjour (RSS), les champs extraits incluent notamment :

| Catégorie           | Champs                                                                           |
|---------------------|----------------------------------------------------------------------------------|
| Identification      | `finess`, `num_rss`, `num_sejour`, `num_rum`, `num_um`                           |
| Patient             | `date_naissance`, `sexe`, `code_postal`, `poids_naissance`, `age_gestationnel`   |
| Séjour              | `date_entree`, `mode_entree`, `provenance`, `date_sortie`, `mode_sortie`, `destination` |
| Groupage            | `cmd`, `ghm`, `code_retour_groupage`, `version_classification`                   |
| Diagnostics         | `dp`, `dr`, `igs2`, `diagnostics_associes[]`, `diagnostics_documentaires[]`      |
| Actes CCAM          | `actes[]` : `code_ccam`, `date_realisation`, `phase`, `activite`, `modificateurs`, `nb_realisations`, … |
| Indicateurs qualité | `conversion_hc`, `raac`, `contexte_patient`, `passage_urgences`, `non_programme`, … |
| Obstétrique (fmt 117) | `nb_ivg_anterieures`, `annee_ivg_precedente`, `nb_naissances_vivantes`        |

Les champs vides ou non renseignés sont **omis** du JSON de sortie.

---

## Installation

### Prérequis

- [Rust](https://rustup.rs/) ≥ 1.80

### Compiler depuis les sources

```bash
git clone https://github.com/citron/pmsi2json.git
cd pmsi2json
cargo build --release
# Le binaire est disponible dans target/release/pmsi2json
```

---

## Utilisation

```
pmsi2json [OPTIONS] <INPUT>

Arguments :
  <INPUT>   Fichier .grp ou répertoire contenant des fichiers .grp

Options :
  -o, --output <FICHIER>   Écrire le JSON dans un fichier (défaut : stdout)
      --pretty             Formater le JSON de façon lisible (indentation)
  -h, --help               Afficher l'aide
  -V, --version            Afficher la version
```

### Exemples

```bash
# Convertir un fichier unique vers stdout
pmsi2json rss_groupe.grp

# Jolie sortie lisible
pmsi2json --pretty rss_groupe.grp

# Écrire le résultat dans un fichier JSON
pmsi2json --pretty rss_groupe.grp -o sortie.json

# Traiter tous les .grp d'un répertoire
pmsi2json --pretty /data/pmsi/2025/

# Rediriger vers jq pour des requêtes ad hoc
pmsi2json rss_groupe.grp | jq '.files[0].records[].dp'
```

### Exemple de sortie JSON

```json
{
  "input": "rss_groupe.grp",
  "files": [
    {
      "path": "rss_groupe.grp",
      "encoding": "utf-8",
      "record_count": 1,
      "records": [
        {
          "ligne": 1,
          "version_classification": "11",
          "cmd": "08",
          "ghm": "08M13J",
          "version_format_rss": "122",
          "code_retour_groupage": "000",
          "finess": "800000017",
          "num_rss": "20250000000001",
          "num_sejour": "20250000000001",
          "date_naissance": "15061975",
          "sexe": "1",
          "date_entree": "10012025",
          "mode_entree": "8",
          "date_sortie": "15012025",
          "mode_sortie": "8",
          "dp": "S72.11",
          "dr": "Z96.64",
          "diagnostics_associes": ["M16.00"],
          "actes": [
            {
              "date_realisation": "10012025",
              "code_ccam": "NEKA011",
              "phase": "0",
              "activite": "1",
              "nb_realisations": "01"
            }
          ]
        }
      ]
    }
  ]
}
```

---

## Format source

Les fichiers `.grp` sont au format **RSS groupé** défini par l'ATIH :

- En-tête fixe de **192 caractères** par ligne (tous formats supportés)
- Puis `nDA × 8` caractères pour les diagnostics associés (CIM-10)
- Puis `nDAD × 8` caractères pour les diagnostics documentaires
- Puis `nZA × za_len` caractères pour les actes CCAM (26 chars en format 116, 29 chars en formats 117+)

Encodage d'origine : **Windows-1252** (avec repli UTF-8 automatique).

Références :
- [Formats PMSI MCO 2025 — ATIH](https://www.atih.sante.fr/formats-pmsi-2025-0)
- [Formats PMSI MCO 2022 — ATIH](https://www.atih.sante.fr/formats-pmsi-2022)
- [Formats PMSI MCO 2016 — ATIH](https://www.atih.sante.fr/node/2833)

---

## Auteur

**William Gacquer** — CHU Amiens  
📧 [gacquer.william@chu-amiens.fr](mailto:gacquer.william@chu-amiens.fr)

---

## Contribuer

Les issues et pull requests sont les bienvenues sur GitHub.  
Ce projet suit les conventions de code Rust standard (`cargo fmt`, `cargo clippy`).
