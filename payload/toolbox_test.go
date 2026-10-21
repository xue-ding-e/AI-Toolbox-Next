package codexsummary

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"strings"
	"testing"
)

// These capsules contain synthetic data only. A fixed nonce belongs in this
// fixture, never in a producer encrypting real history.
func toolboxFixture(t *testing.T, plain []byte) string {
	t.Helper()
	key := sha256.Sum256([]byte("ai-toolbox-remote-compaction"))
	block, err := aes.NewCipher(key[:])
	if err != nil {
		t.Fatal(err)
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		t.Fatal(err)
	}
	nonce := []byte("synthetic123")
	sealed := gcm.Seal(nil, nonce, plain, nil)
	return ToolboxMark + base64.RawURLEncoding.EncodeToString(append(nonce, sealed...))
}

func TestToolboxSummaryPreservesSummary(t *testing.T) {
	summary := "  Synthetic progress: 测试\nNext: keep tool results.\n "
	plain, _ := json.Marshal(map[string]any{"summary": summary, "model": "synthetic", "created_at": 1})
	msg, recognized, err := ToolboxSummary(toolboxFixture(t, plain))
	if err != nil || !recognized || msg != summary {
		t.Fatalf("summary import: recognized=%v err=%v msg=%v", recognized, err, msg)
	}
}

// Independently generated with Python cryptography AESGCM, from the exact JSON
// {"summary":"Synthetic context only","model":"synthetic","created_at":1}
// and nonce synthetic123. Rust consumption is a separate integration check.
func TestToolboxSummaryIndependentVector(t *testing.T) {
	const vector = "ai-toolbox-compact-v1:c3ludGhldGljMTIzlfbqVoqoUJaD8I7eKopBPyXv-z0g3WJrxDTKwYlDdvUl8tUy0EWv8VXm6As41o2CyzfgJdMC9MuL2rW87e12JWkHn3E1O9J1NWPeMomPE19kaFQSluzl"
	summary, recognized, err := ToolboxSummary(vector)
	if err != nil || !recognized || summary != "Synthetic context only" {
		t.Fatalf("independent capsule import: recognized=%v err=%v summary=%q", recognized, err, summary)
	}
}

func TestToolboxSummaryUnreadableIsNotDiscarded(t *testing.T) {
	valid := toolboxFixture(t, []byte(`{"summary":"synthetic","model":"synthetic","created_at":1}`))
	sealed, _ := base64.RawURLEncoding.DecodeString(strings.TrimPrefix(valid, ToolboxMark))
	sealed[len(sealed)-1] ^= 1
	cases := []string{
		ToolboxMark + "%%%", ToolboxMark, ToolboxMark + "AA", valid + "=", valid + "\n",
		ToolboxMark + strings.Repeat("A", toolboxMaxEncoded+1),
		ToolboxMark + base64.RawURLEncoding.EncodeToString(sealed),
	}
	for _, plain := range []string{
		`{"summary":"synthetic"}`, `{"summary":null,"model":"x","created_at":1}`,
		`{"summary":" ","model":"x","created_at":1}`,
		`{"summary":"x","model":"x","created_at":1.5}`,
		`{"summary":"x","model":"x","created_at":1} {}`,
		"{\"summary\":\"\xff\",\"model\":\"x\",\"created_at\":1}",
		`{"summary":"\ud800","model":"x","created_at":1}`,
		`{"summary":"\udc00","model":"x","created_at":1}`,
		`{"summary":"\ud800\u0041","model":"x","created_at":1}`,
		`{"summary":"x","model":"x","created_at":1,"unknown":"\ud800"}`,
	} {
		cases = append(cases, toolboxFixture(t, []byte(plain)))
	}
	for i, value := range cases {
		msg, recognized, err := ToolboxSummary(value)
		if msg != "" || !recognized || err == nil {
			t.Errorf("case %d silently accepted or lost its recognized failure", i)
		}
	}
}

func TestToolboxSummaryPairedUnicode(t *testing.T) {
	for _, plain := range []string{
		`{"summary":"Synthetic \ud83d\ude00","model":"x","created_at":1}`,
		`{"summary":"Synthetic 😀","model":"x","created_at":1,"future":{"data":"\\ud800"}}`,
	} {
		value := toolboxFixture(t, []byte(plain))
		summary, recognized, err := ToolboxSummary(value)
		if err != nil || !recognized || summary != "Synthetic 😀" {
			t.Fatalf("valid non-BMP text or unknown field: recognized=%v err=%v summary=%q", recognized, err, summary)
		}
	}
}

func TestToolboxSummaryLeavesOtherFormats(t *testing.T) {
	for _, value := range []string{"", "opaque-native-synthetic", "unknown-v1:abc", Encrypted("synthetic")} {
		if msg, recognized, err := ToolboxSummary(value); msg != "" || recognized || err != nil {
			t.Errorf("another format classified as Toolbox: recognized=%v err=%v", recognized, err)
		}
	}
}
