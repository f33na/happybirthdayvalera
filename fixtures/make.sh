#!/usr/bin/env bash
# Regenerates the speech fixtures with macOS `say` and `afconvert`: 16 kHz mono 16-bit PCM WAV.
# The phrases and the key words that a recognition test must find are in expected.json.
set -euo pipefail
cd "$(dirname "$0")"

make() {
  local lang=$1 voice=$2 text=$3
  say -v "$voice" -o "$lang.aiff" "$text"
  afconvert -f WAVE -d LEI16@16000 -c 1 "$lang.aiff" "$lang.wav"
  rm "$lang.aiff"
}

make en Samantha "Happy birthday, Valera. The weather is lovely today."
make ru Milena "С днём рождения, Валера. Сегодня прекрасная погода."
make de Anna "Alles Gute zum Geburtstag, Valera. Heute ist das Wetter schön."
make fr Thomas "Joyeux anniversaire, Valera. Il fait très beau aujourd'hui."
