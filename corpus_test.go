package capsule

import (
	"encoding/hex"
	"encoding/json"
	"os"
	"testing"
)

// Harness-only test, kept separate from the frozen candidate's original tests.
func TestSyntheticCorpusInterop(t *testing.T) {
	b, err := os.ReadFile(os.Getenv("CAPSULE_CORPUS"))
	if err != nil { t.Fatal(err) }
	var cases []struct {
		Name string `json:"name"`
		Plain string `json:"plain_hex"`
		GoAccept bool `json:"go_accept"`
		Summary string `json:"summary_hex"`
	}
	if err = json.Unmarshal(b, &cases); err != nil { t.Fatal(err) }
	f, err := os.Create(os.Getenv("CAPSULE_RESULTS"))
	if err != nil { t.Fatal(err) }
	defer f.Close()
	enc := json.NewEncoder(f)
	for _, c := range cases {
		t.Run(c.Name, func(t *testing.T) {
			plain, err := hex.DecodeString(c.Plain)
			if err != nil { t.Fatal(err) }
			summary, err := readSummary(plain)
			accepted := err == nil
			summaryHex := hex.EncodeToString([]byte(summary))
			if accepted != c.GoAccept || (accepted && summaryHex != c.Summary) || (!accepted && summary != "") {
				t.Fatal("synthetic payload classification or summary mismatch")
			}
			if err = enc.Encode(map[string]any{"name": c.Name, "accepted": accepted, "summary_hex": summaryHex}); err != nil { t.Fatal(err) }
		})
	}
}
