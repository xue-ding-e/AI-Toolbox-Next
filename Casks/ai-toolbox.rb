cask "ai-toolbox" do
  version "1.0.8"

  on_arm do
    sha256 "699c2e0ce387c955fa003c931fe7f6144c9b2be224cee7bab380ab6d65d858ac"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.8_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "a9b70afc6c698665e3fe710cb6c8d51ff87721e59c903d3cb505b2611046d107"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.8_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
