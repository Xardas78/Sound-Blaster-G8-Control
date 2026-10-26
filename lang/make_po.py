#!/usr/bin/env python3
"""Baut de/en/fr .po-Dateien aus lang/g8-control.pot, mit fest hinterlegten
Uebersetzungen fuer den Pilot-Wortschatz (settings-tab + sound-mode-tab)."""
import re
from pathlib import Path

BASE = Path("/home/christian/SB G8/g8-scaffold")
POT = BASE / "lang/g8-control.pot"

TRANSLATIONS = {
    "de": {
        # Deutsch ist die Quellsprache -> 1:1 (Identitaet), siehe Gespraech.
    },
    "en": {
        "Suche Gerät …": "Searching for device …",
        "Gerät": "Device",
        "Wiedergabe": "Playback",
        "Aufnahme": "Recording",
        "Einstellungen": "Settings",
        "Aus": "Off",
        "Öffnen": "Open",
        "Beenden": "Quit",
        "Reduzieren Sie statische Hintergrundgeräusche bei ausgehendem Audio während Telefonkonferenzen.":
            "Reduce static background noise on outgoing audio during phone conferences.",
        "Beseitigen Sie Echos, die Ihre Konversation stören.": "Eliminate echoes that disrupt your conversation.",
        "Gut verstanden werden, ohne rufen oder flüstern zu müssen.": "Be understood well without having to shout or whisper.",
        "Morphen Sie Ihre Stimme in verschiedene Charaktere und Akzente.": "Morph your voice into different characters and accents.",
        "Mikrofon-Equalizer": "Microphone Equalizer",
        "Verbessern Sie die Mikrofonleistung und die Qualität Ihrer Stimme.": "Improve microphone performance and the quality of your voice.",
        "Standardquelle": "Default source",
        "✓ Standard": "✓ Default",
        "Standard festlegen": "Set as default",
        "Balance": "Balance",
        "Mitte": "Center",
        "{}% links": "{}% left",
        "{}% rechts": "{}% right",
        "Lautsprecher": "Speakers",
        "Überwachung": "Monitoring",
        "Sound-Modus": "Sound Mode",
        "Equalizer": "Equalizer",
        "Modi & Decoder": "Modes & Decoder",
        "Mixer": "Mixer",
        "Front Links": "Front Left",
        "Front Rechts": "Front Right",
        "Center": "Center",
        "Subwoofer": "Subwoofer",
        "Hinten Links": "Rear Left",
        "Hinten Rechts": "Rear Right",
        "Seite Links": "Side Left",
        "Seite Rechts": "Side Right",
        "Kopfhörer": "Headphones",
        "Virtueller HRTF-Surround (7.1)": "Virtual HRTF surround (7.1)",
        "Stereo, direkt über die G8-Hardware": "Stereo, directly via the G8 hardware",
        "Klicke auf eine Position im Kreis, um einen Testton von dort zu hören. Im Kopfhörer-Modus klingt jede Position dank HRTF-Faltung räumlich unterschiedlich.":
            "Click a position on the circle to hear a test tone from there. In headphone mode, each position sounds spatially different thanks to HRTF convolution.",
        "Testen": "Test",
        "Im Direct Mode erhalten Sie Audio in seiner reinsten Form direkt von der Quelle. Alle Audioeffekte werden nicht auf die Ausgabe angewendet.":
            "In Direct Mode you get audio in its purest form straight from the source. No audio effects are applied to the output.",
        "Entwickelt, um Ihnen zu helfen, das zu hören, was Sie nicht sehen können, wie Schritte, Rede und Waffenwechsel.":
            "Designed to help you hear what you can't see, like footsteps, speech, and weapon switches.",
        "Einige Audioeffekte werden nicht für die Ausgabe übernommen.": "Some audio effects are not applied to the output.",
        "Zugriffstaste": "Access key",
        "Um ein Hotkey zuzuweisen, drücken Sie die Taste auf der Tastatur mit der Kombination von Strg, Alt oder Shift.":
            "To assign a hotkey, press the key on your keyboard together with Ctrl, Alt, or Shift.",
        "Klicken Sie, um den Tastaturanschlag zuzuweisen": "Click to assign the key combination",
        "Decoder": "Decoder",
        "Spielen Sie Ihre Dolby Digital Medien einfach mit dem integrierten Decoder.":
            "Play your Dolby Digital media easily with the built-in decoder.",
        "Normal": "Normal",
        "Voll": "Full",
        "Nacht": "Night",
        "Kopfhörer aktiviert virtuellen HRTF-Surround (SBX) – PipeWire startet dafür kurz neu.":
            "Headphones enable virtual HRTF surround (SBX) -- PipeWire briefly restarts for this.",
        "Lautsprecher gibt direkt über die G8-Hardware aus, ohne virtuellen Surround.":
            "Speakers output directly via the G8 hardware, without virtual surround.",
        "Erzeugt ein beispielloses Maß an Audio-Realismus, indem virtuelle Lautsprecher um Sie herum erzeugt werden.":
            "Creates an unprecedented level of audio realism by generating virtual speakers around you.",
        "Stellt sicher, dass Musik so klingt, wie der Künstler ursprünglich beabsichtigt hat, indem man jedes Audio-Detail hören kann.":
            "Ensures music sounds the way the artist originally intended by letting you hear every audio detail.",
        "Deutlich verbessert das Klangerlebnis durch die bessere Wiedergabe der tiefen Töne.":
            "Significantly improves the sound experience through better reproduction of low tones.",
        "Laut": "Loud",
        "Intelligenterweise verringert abrupte Lautstärkeänderungen durch automatische Lautstärkeanpassung der Audiowiedergabe.":
            "Intelligently reduces abrupt volume changes through automatic volume adjustment of audio playback.",
        "Intelligenterweise verstärkt Stimmen in Musik und Filmen für eine deutlich klarere Stimmenwiedergabe.":
            "Intelligently boosts voices in music and movies for noticeably clearer voice reproduction.",
        "Eigenes 1": "Custom 1",
        "Eigenes 2": "Custom 2",
        "Eigenes 3": "Custom 3",
        "Vorverstärker": "Preamp",
        "Bass": "Bass",
        "Höhen": "Treble",
        "EQ-Preset": "EQ preset",
        "Speichern": "Save",
        "Im Gerät": "On device",
        "Prüfung fehlgeschlagen": "Check failed",
        "Update verfügbar": "Update available",
        "Keine neue Firmware": "No new firmware",
        "Der Update-Server war nicht erreichbar (keine Internetverbindung?). Bitte später erneut versuchen.":
            "The update server was unreachable (no internet connection?). Please try again later.",
        "Eine neuere Firmware ({}) ist verfügbar. Firmware-Updates werden von dieser App nicht unterstützt – bitte dafür die offizielle Creative-Software unter Windows verwenden.":
            "A newer firmware ({}) is available. Firmware updates are not supported by this app -- please use the official Creative software on Windows for that.",
        "Es ist keine neuere Firmware verfügbar.": "No newer firmware is available.",
        "OK": "OK",
        "Verbunden (USB 041e:329b)": "Connected (USB 041e:329b)",
        "Nicht verbunden": "Not connected",
        "S/N: {}": "S/N: {}",
        "Gerät zurücksetzen": "Reset device",
        "Nach Firmware-Update suchen": "Check for firmware update",
        "Firmware: {}": "Firmware: {}",
        "Version {}": "Version {}",
        "Energieeinsparung": "Power saving",
        "Sound Blaster Logo LED": "Sound Blaster Logo LED",
        "Anwendung": "Application",
        "Dark Mode": "Dark Mode",
        "Mit dem System starten": "Start with the system",
        "Beim Schließen": "When closing",
        "Sprache": "Language",
        "✗ G8 nicht gefunden – ist das Gerät angeschlossen?": "✗ G8 not found – is the device connected?",
        "Automatisch (System)": "Automatic (system)",
        "Schließen": "Close",
        "In den Tray minimieren": "Minimize to tray",
        "Lautstärke-Synchronisierung": "Volume synchronization",
        "Wird nicht benötigt: dein System hält Gerät- und PC-Lautstärke bereits automatisch synchron, auch wenn diese App nicht läuft.":
            "Not needed: your system already keeps device and PC volume in sync automatically, even when this app isn't running.",
        "Sound-Modus": "Sound Mode",
        "Im Gerät": "On device",
        "Kopfhörer": "Headphones",
        "Lautsprecher": "Speakers",
        "Aus": "Off",
    },
    "fr": {
        "Suche Gerät …": "Recherche de l'appareil …",
        "Gerät": "Appareil",
        "Wiedergabe": "Lecture",
        "Aufnahme": "Enregistrement",
        "Einstellungen": "Paramètres",
        "Aus": "Désactivé",
        "Öffnen": "Ouvrir",
        "Beenden": "Quitter",
        "Reduzieren Sie statische Hintergrundgeräusche bei ausgehendem Audio während Telefonkonferenzen.":
            "Réduisez les bruits de fond statiques sur l'audio sortant lors des téléconférences.",
        "Beseitigen Sie Echos, die Ihre Konversation stören.": "Éliminez les échos qui perturbent votre conversation.",
        "Gut verstanden werden, ohne rufen oder flüstern zu müssen.": "Soyez bien compris sans avoir à crier ou chuchoter.",
        "Morphen Sie Ihre Stimme in verschiedene Charaktere und Akzente.": "Transformez votre voix en différents personnages et accents.",
        "Mikrofon-Equalizer": "Égaliseur du microphone",
        "Verbessern Sie die Mikrofonleistung und die Qualität Ihrer Stimme.": "Améliorez les performances du microphone et la qualité de votre voix.",
        "Standardquelle": "Source par défaut",
        "✓ Standard": "✓ Par défaut",
        "Standard festlegen": "Définir par défaut",
        "Balance": "Balance",
        "Mitte": "Centre",
        "{}% links": "{}% à gauche",
        "{}% rechts": "{}% à droite",
        "Lautsprecher": "Haut-parleurs",
        "Überwachung": "Surveillance",
        "Sound-Modus": "Mode sonore",
        "Equalizer": "Égaliseur",
        "Modi & Decoder": "Modes et décodeur",
        "Mixer": "Mixeur",
        "Front Links": "Avant gauche",
        "Front Rechts": "Avant droit",
        "Center": "Centre",
        "Subwoofer": "Caisson de basses",
        "Hinten Links": "Arrière gauche",
        "Hinten Rechts": "Arrière droit",
        "Seite Links": "Latéral gauche",
        "Seite Rechts": "Latéral droit",
        "Kopfhörer": "Casque",
        "Virtueller HRTF-Surround (7.1)": "Surround virtuel HRTF (7.1)",
        "Stereo, direkt über die G8-Hardware": "Stéréo, directement via le matériel G8",
        "Klicke auf eine Position im Kreis, um einen Testton von dort zu hören. Im Kopfhörer-Modus klingt jede Position dank HRTF-Faltung räumlich unterschiedlich.":
            "Cliquez sur une position dans le cercle pour entendre une tonalité de test depuis cet endroit. En mode casque, chaque position sonne différemment dans l'espace grâce à la convolution HRTF.",
        "Testen": "Tester",
        "Im Direct Mode erhalten Sie Audio in seiner reinsten Form direkt von der Quelle. Alle Audioeffekte werden nicht auf die Ausgabe angewendet.":
            "En mode direct, vous obtenez l'audio sous sa forme la plus pure directement depuis la source. Aucun effet audio n'est appliqué à la sortie.",
        "Entwickelt, um Ihnen zu helfen, das zu hören, was Sie nicht sehen können, wie Schritte, Rede und Waffenwechsel.":
            "Conçu pour vous aider à entendre ce que vous ne pouvez pas voir, comme les pas, la parole et les changements d'armes.",
        "Einige Audioeffekte werden nicht für die Ausgabe übernommen.": "Certains effets audio ne sont pas appliqués à la sortie.",
        "Zugriffstaste": "Touche d'accès",
        "Um ein Hotkey zuzuweisen, drücken Sie die Taste auf der Tastatur mit der Kombination von Strg, Alt oder Shift.":
            "Pour attribuer un raccourci, appuyez sur la touche du clavier en combinaison avec Ctrl, Alt ou Maj.",
        "Klicken Sie, um den Tastaturanschlag zuzuweisen": "Cliquez pour attribuer la combinaison de touches",
        "Decoder": "Décodeur",
        "Spielen Sie Ihre Dolby Digital Medien einfach mit dem integrierten Decoder.":
            "Lisez facilement vos médias Dolby Digital avec le décodeur intégré.",
        "Normal": "Normal",
        "Voll": "Complet",
        "Nacht": "Nuit",
        "Kopfhörer aktiviert virtuellen HRTF-Surround (SBX) – PipeWire startet dafür kurz neu.":
            "Le casque active le surround virtuel HRTF (SBX) -- PipeWire redémarre brièvement pour cela.",
        "Lautsprecher gibt direkt über die G8-Hardware aus, ohne virtuellen Surround.":
            "Les haut-parleurs sortent directement via le matériel G8, sans surround virtuel.",
        "Erzeugt ein beispielloses Maß an Audio-Realismus, indem virtuelle Lautsprecher um Sie herum erzeugt werden.":
            "Crée un niveau de réalisme audio sans précédent en générant des haut-parleurs virtuels autour de vous.",
        "Stellt sicher, dass Musik so klingt, wie der Künstler ursprünglich beabsichtigt hat, indem man jedes Audio-Detail hören kann.":
            "Garantit que la musique sonne comme l'artiste l'a voulu à l'origine, en vous permettant d'entendre chaque détail audio.",
        "Deutlich verbessert das Klangerlebnis durch die bessere Wiedergabe der tiefen Töne.":
            "Améliore nettement l'expérience sonore grâce à une meilleure reproduction des sons graves.",
        "Laut": "Fort",
        "Intelligenterweise verringert abrupte Lautstärkeänderungen durch automatische Lautstärkeanpassung der Audiowiedergabe.":
            "Réduit intelligemment les changements de volume abrupts grâce à un ajustement automatique du volume de lecture audio.",
        "Intelligenterweise verstärkt Stimmen in Musik und Filmen für eine deutlich klarere Stimmenwiedergabe.":
            "Amplifie intelligemment les voix dans la musique et les films pour une restitution vocale nettement plus claire.",
        "Eigenes 1": "Personnalisé 1",
        "Eigenes 2": "Personnalisé 2",
        "Eigenes 3": "Personnalisé 3",
        "Vorverstärker": "Préamplificateur",
        "Bass": "Graves",
        "Höhen": "Aigus",
        "EQ-Preset": "Préréglage EQ",
        "Speichern": "Enregistrer",
        "Im Gerät": "Sur l'appareil",
        "Prüfung fehlgeschlagen": "Échec de la vérification",
        "Update verfügbar": "Mise à jour disponible",
        "Keine neue Firmware": "Aucun nouveau micrologiciel",
        "Der Update-Server war nicht erreichbar (keine Internetverbindung?). Bitte später erneut versuchen.":
            "Le serveur de mise à jour était injoignable (pas de connexion Internet ?). Veuillez réessayer plus tard.",
        "Eine neuere Firmware ({}) ist verfügbar. Firmware-Updates werden von dieser App nicht unterstützt – bitte dafür die offizielle Creative-Software unter Windows verwenden.":
            "Un micrologiciel plus récent ({}) est disponible. Les mises à jour du micrologiciel ne sont pas prises en charge par cette application -- veuillez utiliser le logiciel officiel Creative sous Windows à cet effet.",
        "Es ist keine neuere Firmware verfügbar.": "Aucun micrologiciel plus récent n'est disponible.",
        "OK": "OK",
        "Verbunden (USB 041e:329b)": "Connecté (USB 041e:329b)",
        "Nicht verbunden": "Non connecté",
        "S/N: {}": "N° de série : {}",
        "Gerät zurücksetzen": "Réinitialiser l'appareil",
        "Nach Firmware-Update suchen": "Rechercher une mise à jour du micrologiciel",
        "Firmware: {}": "Micrologiciel : {}",
        "Version {}": "Version {}",
        "Energieeinsparung": "Économie d'énergie",
        "Sound Blaster Logo LED": "LED du logo Sound Blaster",
        "Anwendung": "Application",
        "Dark Mode": "Mode sombre",
        "Mit dem System starten": "Démarrer avec le système",
        "Beim Schließen": "À la fermeture",
        "Sprache": "Langue",
        "✗ G8 nicht gefunden – ist das Gerät angeschlossen?": "✗ G8 introuvable – l'appareil est-il connecté ?",
        "Automatisch (System)": "Automatique (système)",
        "Schließen": "Fermer",
        "In den Tray minimieren": "Réduire dans la barre d'état système",
        "Lautstärke-Synchronisierung": "Synchronisation du volume",
        "Wird nicht benötigt: dein System hält Gerät- und PC-Lautstärke bereits automatisch synchron, auch wenn diese App nicht läuft.":
            "Non nécessaire : votre système synchronise déjà automatiquement le volume de l'appareil et du PC, même lorsque cette application n'est pas en cours d'exécution.",
        "Sound-Modus": "Mode sonore",
        "Im Gerät": "Sur l'appareil",
        "Kopfhörer": "Casque",
        "Lautsprecher": "Haut-parleurs",
        "Aus": "Désactivé",
    },
}

HEADER_TEMPLATE = '''# {lang_name} translation for g8-control.
#
msgid ""
msgstr ""
"Project-Id-Version: g8-control 0.1.1\\n"
"PO-Revision-Date: 2026-08-25\\n"
"Language: {lang}\\n"
"MIME-Version: 1.0\\n"
"Content-Type: text/plain; charset=UTF-8\\n"
"Content-Transfer-Encoding: 8bit\\n"
"Plural-Forms: nplurals=1; plural=0;\\n"

'''

LANG_NAMES = {"de": "German", "en": "English", "fr": "French"}


def parse_pot(text):
    """Liefert Liste von (comment_lines, msgctxt, msgid) in Original-Reihenfolge."""
    entries = []
    blocks = text.split("\n\n")
    for block in blocks[1:]:  # erster Block ist der Header
        lines = block.strip("\n").splitlines()
        comments = [l for l in lines if l.startswith("#:")]
        ctx_match = re.search(r'msgctxt "((?:[^"\\]|\\.)*)"', block)
        ctx = ctx_match.group(1) if ctx_match else None
        # msgid kann mehrzeilig sein (Python-Style String-Concat der Quelllines)
        id_start = block.index('msgid "')
        rest = block[id_start + len('msgid "'):]
        id_end = rest.index('"\nmsgstr')
        raw_id = rest[:id_end]
        # mehrzeilige msgid: "..." \n "..." zusammenfuegen
        raw_id_full = re.search(r'msgid ((?:"(?:[^"\\]|\\.)*"\s*)+)', block).group(1)
        pieces = re.findall(r'"((?:[^"\\]|\\.)*)"', raw_id_full)
        msgid = "".join(p.replace('\\n', '\n').replace('\\"', '"') for p in pieces)
        if not msgid:
            continue
        entries.append((comments, ctx, msgid))
    return entries


def po_escape(s):
    return s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")


def write_po(lang, entries, translations):
    out_dir = BASE / "lang" / lang / "LC_MESSAGES"
    out_dir.mkdir(parents=True, exist_ok=True)
    out_file = out_dir / "g8-control.po"
    parts = [HEADER_TEMPLATE.format(lang=lang, lang_name=LANG_NAMES[lang])]
    for comments, ctx, msgid in entries:
        block_lines = []
        block_lines.extend(comments)
        if ctx:
            block_lines.append(f'msgctxt "{po_escape(ctx)}"')
        block_lines.append(f'msgid "{po_escape(msgid)}"')
        translated = translations.get(msgid, msgid)  # de: Identitaet als Fallback
        block_lines.append(f'msgstr "{po_escape(translated)}"')
        parts.append("\n".join(block_lines) + "\n")
    out_file.write_text("\n".join(parts))
    print(f"{out_file}: {len(entries)} Einträge")


def main():
    pot_text = POT.read_text()
    entries = parse_pot(pot_text)
    print(f"{len(entries)} Strings aus POT geparst")
    for lang, translations in TRANSLATIONS.items():
        write_po(lang, entries, translations)


if __name__ == "__main__":
    main()
