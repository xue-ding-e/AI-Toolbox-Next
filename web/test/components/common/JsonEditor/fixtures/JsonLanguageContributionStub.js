// The blur fixture only exercises editor focus/blur plumbing, so it replaces
// the JSON language contribution with this no-op: registering the real
// contribution would make Monaco request a JSON worker, and the browser fixture
// (unlike the app's `web/app/monaco.ts`) installs no worker factory.
export {};