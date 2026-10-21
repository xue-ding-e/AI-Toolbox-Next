package codexsummary

import (
	"bytes"
	"crypto/aes"
	"crypto/cipher"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"strings"
	"unicode/utf8"
)

// ToolboxMark identifies AI Toolbox's public, portable compaction envelope.
const ToolboxMark = "ai-toolbox-compact-v1:"

// Bound allocations before decoding untrusted history. An oversized capsule is
// reported as unreadable, never truncated or discarded.
const toolboxMaxEncoded = 16 << 20

// ToolboxSummary reads AI Toolbox's v1 capsule without changing any history.
// The format is defined by ai-toolbox's codex_remote_compaction.rs: a SHA-256
// derived public format key, AES-256-GCM with empty AAD, and nonce || ciphertext
// || tag encoded as unpadded URL-safe base64. This is not an account credential.
// A recognized but unreadable capsule returns an error: callers must retain the
// original and report the failure, not drop it or forward it as native ciphertext.
// No gateway or rollout writer calls this importer until publication can also
// preserve Codex's indexes, fork references and an original backup.
func ToolboxSummary(encrypted string) (summary string, recognized bool, err error) {
	body, ok := strings.CutPrefix(encrypted, ToolboxMark)
	if !ok {
		return "", false, nil
	}
	bad := func() (string, bool, error) {
		return "", true, errors.New("unreadable AI Toolbox compaction capsule; original must be retained")
	}
	if len(body) > toolboxMaxEncoded || strings.ContainsAny(body, "\r\n") {
		return bad()
	}
	sealed, err := base64.RawURLEncoding.Strict().DecodeString(body)
	if err != nil || len(sealed) < 12+16 {
		return bad()
	}
	key := sha256.Sum256([]byte("ai-toolbox-remote-compaction"))
	block, err := aes.NewCipher(key[:])
	if err != nil {
		return bad()
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return bad()
	}
	plain, err := gcm.Open(nil, sealed[:12], sealed[12:], nil)
	if err != nil || !utf8.Valid(plain) || !toolboxJSONStringsValid(plain) {
		return bad()
	}
	var capsule struct {
		Summary   *string `json:"summary"`
		Model     *string `json:"model"`
		CreatedAt *int64  `json:"created_at"`
	}
	d := json.NewDecoder(bytes.NewReader(plain))
	if d.Decode(&capsule) != nil || capsule.Summary == nil || capsule.Model == nil || capsule.CreatedAt == nil || strings.TrimSpace(*capsule.Summary) == "" {
		return bad()
	}
	var extra any
	if d.Decode(&extra) != io.EOF {
		return bad()
	}
	// Keep the summary's original whitespace, unlike Toolbox's trim on replay.
	return *capsule.Summary, true, nil
}

// encoding/json replaces unpaired UTF-16 escapes with U+FFFD rather than
// rejecting them. A damaged summary must not be silently changed on import.
// json.Valid first establishes every escape's syntax and bounds; this scan
// checks the extra Unicode scalar constraint in every string, including keys
// and unknown fields. Escaped backslashes are consumed without interpreting
// the following literal characters as another escape.
func toolboxJSONStringsValid(b []byte) bool {
	if !json.Valid(b) {
		return false
	}
	hex4 := func(at int) uint16 {
		var v uint16
		for _, c := range b[at : at+4] {
			v *= 16
			switch {
			case c >= '0' && c <= '9':
				v += uint16(c - '0')
			case c >= 'a' && c <= 'f':
				v += uint16(c-'a') + 10
			default:
				v += uint16(c-'A') + 10
			}
		}
		return v
	}
	for i := 0; i < len(b); i++ {
		if b[i] != '"' {
			continue
		}
		for i++; b[i] != '"'; {
			if b[i] != '\\' {
				i++
				continue
			}
			i++
			if b[i] != 'u' {
				i++
				continue
			}
			v := hex4(i + 1)
			i += 5
			if v >= 0xdc00 && v <= 0xdfff {
				return false
			}
			if v >= 0xd800 && v <= 0xdbff {
				if i+6 > len(b) || b[i] != '\\' || b[i+1] != 'u' {
					return false
				}
				low := hex4(i + 2)
				if low < 0xdc00 || low > 0xdfff {
					return false
				}
				i += 6
			}
		}
	}
	return true
}
