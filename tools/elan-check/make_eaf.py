"""Writes a small ELAN annotation file (.eaf) of invented material, with a recording beside it.

usage: python make_eaf.py <folder> [<name of the recording file>]     (the folder gets the recording, `Bus ride talk.wav` unless another name is given, and `bus-ride-talk.eaf`)

The file has what ELAN's own REFI (QDPX) export is made to meet: two speaker tiers (Interviewer and Participant, each with a participant
code and an annotator), a coding tier that depends on the Participant tier (a symbolic association, one code per utterance, from a controlled
vocabulary), a coding tier of its own that codes stretches of the recording directly, and a controlled vocabulary with an entry no annotation uses.
Accents, an emoji, and Chinese are in the words on purpose. Nothing here is real.
"""
import os
import shutil
import sys
from xml.sax.saxutils import escape

out = os.path.abspath(sys.argv[1])
os.makedirs(out, exist_ok=True)
src = os.path.join(os.environ.get("QUALCODER_CHECK_DIR") or os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "qualcoder-test")), "work", "Bus ride talk.wav")   # a made-up 20 second recording, from the QualCoder check
wav_name = sys.argv[2] if len(sys.argv) > 2 else "Bus ride talk.wav"
wav = os.path.join(out, wav_name)
shutil.copyfile(src, wav)

# (id, tier, start ms, end ms, text)
speech = [
    ("a1", "Interviewer", 0, 4000, "How do you usually get to work?"),
    ("a2", "Participant", 4200, 8800, "I take the bus, but it is often late and the fare keeps rising."),
    ("a3", "Interviewer", 9000, 12000, "And does it run on time?"),
    ("a4", "Participant", 12200, 15800, "Not really: I missed a connection twice this month. Caf\u00e9 men\u00fc \u2615 \u4e2d\u6587"),
    ("a5", "Interviewer", 16000, 19000, "Do you feel safe waiting there at night?"),
]
coding_under_participant = [("a6", "a2", "Cost"), ("a7", "a4", "Reliability")]   # one code per utterance, symbolic association
free_coding = [("a8", 16000, 19800, "Safety")]                                      # a code on a stretch of the recording itself

slots = {}
def slot(ms):
    if ms not in slots:
        slots[ms] = f"ts{len(slots) + 1}"
    return slots[ms]
for _, _, s, e, _ in speech:
    slot(s); slot(e)
for _, s, e, _ in free_coding:
    slot(s); slot(e)

x = []
x.append('<?xml version="1.0" encoding="UTF-8"?>')
x.append('<ANNOTATION_DOCUMENT AUTHOR="Ada Lovelace" DATE="2026-11-12T12:00:00+00:00" FORMAT="3.0" VERSION="3.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="http://www.mpi.nl/tools/elan/EAFv3.0.xsd">')
x.append('    <HEADER MEDIA_FILE="" TIME_UNITS="milliseconds">')
url = "file:///" + wav.replace("\\", "/").replace(" ", "%20")
x.append(f'        <MEDIA_DESCRIPTOR MEDIA_URL="{url}" MIME_TYPE="audio/x-wav" RELATIVE_MEDIA_URL="./{wav_name.replace(" ", "%20")}"/>')
x.append('        <PROPERTY NAME="lastUsedAnnotationId">8</PROPERTY>')
x.append('    </HEADER>')
x.append('    <TIME_ORDER>')
for ms, name in sorted(slots.items()):
    x.append(f'        <TIME_SLOT TIME_SLOT_ID="{name}" TIME_VALUE="{ms}"/>')
x.append('    </TIME_ORDER>')

def aligned(a):
    i, _, s, e, t = a
    return (f'        <ANNOTATION>\n            <ALIGNABLE_ANNOTATION ANNOTATION_ID="{i}" TIME_SLOT_REF1="{slot(s)}" TIME_SLOT_REF2="{slot(e)}">\n'
            f'                <ANNOTATION_VALUE>{escape(t)}</ANNOTATION_VALUE>\n            </ALIGNABLE_ANNOTATION>\n        </ANNOTATION>')

for tier, who, annotator in (("Interviewer", "INT", "Ada Lovelace"), ("Participant", "PAR", "Sam Okafor")):
    x.append(f'    <TIER ANNOTATOR="{annotator}" LINGUISTIC_TYPE_REF="speech" PARTICIPANT="{who}" TIER_ID="{tier}">')
    for a in speech:
        if a[1] == tier:
            x.append(aligned(a))
    x.append('    </TIER>')
x.append('    <TIER ANNOTATOR="Ada Lovelace" LINGUISTIC_TYPE_REF="theme" PARENT_REF="Participant" TIER_ID="Theme">')
for i, ref, t in coding_under_participant:
    x.append(f'        <ANNOTATION>\n            <REF_ANNOTATION ANNOTATION_ID="{i}" ANNOTATION_REF="{ref}">\n                <ANNOTATION_VALUE>{t}</ANNOTATION_VALUE>\n            </REF_ANNOTATION>\n        </ANNOTATION>')
x.append('    </TIER>')
x.append('    <TIER ANNOTATOR="Sam Okafor" LINGUISTIC_TYPE_REF="free" TIER_ID="Safety">')
for i, s, e, t in free_coding:
    x.append(aligned((i, "Safety", s, e, t)))
x.append('    </TIER>')
x.append('    <LINGUISTIC_TYPE GRAPHIC_REFERENCES="false" LINGUISTIC_TYPE_ID="speech" TIME_ALIGNABLE="true"/>')
x.append('    <LINGUISTIC_TYPE CONSTRAINTS="Symbolic_Association" CONTROLLED_VOCABULARY_REF="themes" GRAPHIC_REFERENCES="false" LINGUISTIC_TYPE_ID="theme" TIME_ALIGNABLE="false"/>')
x.append('    <LINGUISTIC_TYPE CONTROLLED_VOCABULARY_REF="themes" GRAPHIC_REFERENCES="false" LINGUISTIC_TYPE_ID="free" TIME_ALIGNABLE="true"/>')
x.append('    <LOCALE COUNTRY_CODE="US" LANGUAGE_CODE="en"/>')
x.append('    <LANGUAGE LANG_DEF="http://cdb.iso.org/lg/CDB-00130975-001" LANG_ID="und" LANG_LABEL="undetermined (und)"/>')
x.append('    <CONSTRAINT DESCRIPTION="Time subdivision of parent annotation\'s time interval, no time gaps allowed within this interval" STEREOTYPE="Time_Subdivision"/>')
x.append('    <CONSTRAINT DESCRIPTION="Symbolic subdivision of a parent annotation. Annotations refering to the same parent are ordered" STEREOTYPE="Symbolic_Subdivision"/>')
x.append('    <CONSTRAINT DESCRIPTION="1-1 association with a parent annotation" STEREOTYPE="Symbolic_Association"/>')
x.append('    <CONSTRAINT DESCRIPTION="Time alignable annotations within the parent annotation\'s time interval, gaps are allowed" STEREOTYPE="Included_In"/>')
x.append('    <CONTROLLED_VOCABULARY CV_ID="themes">')
x.append('        <DESCRIPTION LANG_REF="und">Themes of the study</DESCRIPTION>')
for n, t in enumerate(("Cost", "Reliability", "Safety", "Accessibility"), 1):
    x.append(f'        <CV_ENTRY_ML CVE_ID="cveid{n}">\n            <CVE_VALUE DESCRIPTION="" LANG_REF="und">{t}</CVE_VALUE>\n        </CV_ENTRY_ML>')
x.append('    </CONTROLLED_VOCABULARY>')
x.append('</ANNOTATION_DOCUMENT>')
eaf = os.path.join(out, "bus-ride-talk.eaf")
open(eaf, "w", encoding="utf-8", newline="\n").write("\n".join(x) + "\n")
print("wrote", eaf, "and", wav)
