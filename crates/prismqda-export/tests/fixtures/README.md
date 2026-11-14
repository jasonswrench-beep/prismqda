# Test files

- `elan-7.1-bus-ride-talk.qdpx` is a file written by **ELAN 7.1's own REFI-QDA writer** (the class behind *File > Export Multiple Files As > REFI Project File...*),
  run on an annotation file of invented material (`tools/elan-check/make_eaf.py`: two speaker tiers, a coding tier under one of them, a free coding tier, and a
  vocabulary), with the transcript tiers `Interviewer` and `Participant` and the coding tiers `Theme` and `Safety`, the recording left outside the archive.
  The only change made afterwards is the folder of the recording in its two `absolute://` addresses, which was a folder of the computer that made it and is now
  `C:/Recordings`. Its shape is ELAN's: the project file is named `Bus ride talk.qde` and not `project.qde`, the folder is `Sources`, the name of the
  recording keeps its `%20`, and the transcript file is named by the recording's transcript and again by a text source that holds the codings of its words.
