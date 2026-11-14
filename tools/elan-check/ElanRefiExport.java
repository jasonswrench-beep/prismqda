// Runs ELAN 7.1's own QDPX writer (the class behind File > Export Multiple Files As > REFI Project File...) on one .eaf file, with no window.
//
//   java -Djava.awt.headless=true -cp "<ELAN app folder>\*;<this class' folder>" ElanRefiExport <in.eaf, or several separated by ;> <out.qdpx> <bundle|link> <transcript tiers, comma separated> <coding tiers, comma separated>
//
// It does what the export window's last step does (MultipleFileREFIExportStep3): load each file with TranscriptionImpl, call REFIEncoder.encode, then saveAndClose,
// with the settings of the window's second step set from the arguments below. `bundle` copies the recording into the package, `link` leaves it where it is.
import java.io.FileOutputStream;
import java.util.Arrays;
import java.util.List;
import java.util.zip.ZipOutputStream;

import mpi.eudico.server.corpora.clomimpl.abstr.TranscriptionImpl;
import mpi.eudico.server.corpora.clomimpl.refi.REFIEncoder;
import mpi.eudico.server.corpora.clomimpl.refi.REFIEncoderInfo;

public class ElanRefiExport {
    public static void main(String[] args) throws Exception {
        String eaf = args[0];
        String out = args[1];
        boolean bundle = args[2].equals("bundle");
        List<String> transcript = args[3].isEmpty() ? List.of() : Arrays.asList(args[3].split(","));
        List<String> coding = args[4].isEmpty() ? List.of() : Arrays.asList(args[4].split(","));

        REFIEncoderInfo info = new REFIEncoderInfo();
        info.setProjectName("Bus ride talk");
        info.setOrigin("ELAN 7.1");
        info.setTranscriptTiers(transcript);
        info.setCodingTiers(coding);
        info.setIncludeSpeakerLabels(true);
        info.setRepeatSameSpeakerLabel(true);
        info.setIncludeBeginTimeStamps(true);
        info.setIncludeEndTimeStamps(false);
        info.setMergeSameSpeakerAnnotations(false);
        info.setInsertSilenceLines(false);
        info.setIncludeMediaFilesInBundle(bundle);

        try (ZipOutputStream zip = new ZipOutputStream(new FileOutputStream(out))) {
            info.setQdpxFile(zip);
            REFIEncoder encoder = new REFIEncoder(info);
            for (String one : eaf.split(";")) {
                encoder.encode(new TranscriptionImpl(one.replace("\\", "/")));
            }
            encoder.saveAndClose();
            zip.finish();
        }
        System.out.println("wrote " + out);
    }
}
