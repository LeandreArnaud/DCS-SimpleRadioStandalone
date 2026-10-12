# Aéronefs supportés

Un fichier Lua par avion dans `Scripts/DCS-SRS/Scripts/DCS-SRS-Modules/`. Recette pour en ajouter
un : `docs-agents/guidelines/recettes.md`. Détails du contrat Lua : `docs-agents/blocs/lua-dcs-integration.md`.

## Modules présents (2026-10)

A10A, A10C, A10C2, A29B, A4E, AH64, AH6J, AJS37, AV8BNA, Bf109, C101CC, C101EB, C130J30, CH47F,
EagleII, F100D, F14, F15C, F15ESE, F16C, F1BE, F1CE, F4, F4U, F5E, F86, FA18C, FBMkVI, FW190, Hawk,
Hercules, JF17, KA50, L39, La7, LFMkIX, M2000C, MB339A, Mi24P, Mi8, MiG15, MiG19, MiG21, MiG29,
MiG29Fulcrum, OH58D, OH6A, P47, P51, PUCARA, SA342, SH60B, SK60, Su25, Su27, T45, UH1H, UH60L,
VSNF104C, VSNF4, Yak52.

Les avions FC3 (F-15C, Su-27, Su-33, Su-25, MiG-29, A-10A, J-11) et le slot spectateur/GCI utilisent
les radios « overlay » : fréquence et volume réglés depuis SRS.

## Chiffrement par avion (résumé wiki)

| Avion | Radios chiffrables | Clés | Commande |
|-------|-------------------|------|----------|
| A-10C II | UHF, VHF FM (exclusif) | 1-6 | KY-58 : OP, clé, C/RAD 1 ou 2. HQ via sélecteur 100 MHz sur A |
| AH-64D | UHF, FM, HF | — | Page COM → SET → CIPHER/PLAIN + clé, ou édition de preset |
| AV-8B | UHF, V/UHF (simultané) | 1-6 | UFC COMM1/2, ODU 4 PLN→CIPH, ODU 5 clé |
| CH-47F | via page CNI | — | COMSEC MODE CIPHER ; clé via overlay |
| F-5E | UHF | HQ seulement | Bouton gauche UHF sur A, mode P |
| F-14 A/B | UHF, V/UHF (exclusif) | 1-8 | KY-28 en RIO ; délai 1-2 s avant émission |
| F-16C | UHF, V/UHF (exclusif) | 1-6 | KY-58 ON, mode C, clé, CRAD1/2 |
| F/A-18C | UHF | 1-6 | Même KY-58 que F-16 |
| JF-17 | COMM1/2 | — | CMS = chiffré, TRS = HQ + chiffré, PLN = clair |
| M-2000C | UHF | 1-252 | Bouton vert CDE ; clé via overlay |
| SA342 | UHF, VHF FM | 1-252 | UHF mode NA, VHF mode CH.OTAN ; clé via overlay |
| AJS-37, L-39 | aucune | — | Overlay si radios d'expansion |
| Ka-50, C-101, Mi-8, Mi-24, UH-1H | aucune | — | — |

Ces informations viennent du wiki et sont partiellement empiriques.

## Noms de profils auto (tableau de score DCS)

Avions : A10 (A/C/C2), AJS37, AV8B, BF109, C101, F14 (A/B, `_2` RIO), F15C, F15E (`_2` WSO), F16, F5E,
F86F, FA18C, FW190 (A8/D9), JF17, L39, M2000C, MIG19, MIG21, MIG29 (A/S/G), P47, P51D, SPITFIRE,
SU25 (T), SU27, SU33, YAK52.
Hélicos : AH64D (`_2` front, seulement en vrai bi-place), CH47F, KA50, MI8, OH58, SA342 (L/M), UH1H.
