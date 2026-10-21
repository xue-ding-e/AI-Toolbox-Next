const baseUrl = 'https://desktop-fixture.example.invalid';
const token = 'synthetic-not-a-real-credential';
const envBoth = { ANTHROPIC_BASE_URL: baseUrl, ANTHROPIC_AUTH_TOKEN: token, ANTHROPIC_DEFAULT_SONNET_MODEL: 'claude-sonnet-5', ANTHROPIC_DEFAULT_SONNET_MODEL_NAME: 'Synthetic Sonnet' };
const candidate = env => ({
 providerId: 'ccs:claude:synthetic-origin', rawId: 'synthetic-origin', sourceProviderId: 'ccs:claude:synthetic-origin',
 name: 'Synthetic imported official', appType: 'claude', category: 'official', normalizedCategory: 'official',
 settingsConfig: JSON.stringify({env}), extraSettingsConfig: '{}', notes: 'Synthetic source note',
 websiteUrl: 'https://vendor.example.invalid', icon: 'Anthropic', iconColor: '#123456',
 hasApiKey: Boolean(env.ANTHROPIC_AUTH_TOKEN), isLocalEndpoint: false,
});
const record = { id: 'synthetic-existing', name: 'Synthetic existing', category: 'official', settingsConfig: JSON.stringify({env:envBoth}), notes: 'Synthetic source note', isApplied: false, isDisabled: false, createdAt: '2026-01-01T00:00:00Z', updatedAt: '2026-01-01T00:00:00Z' };
export const scenarios = [
 { id:'official-both-unchanged', candidate:candidate(envBoth), action:'unchanged', red:true },
 { id:'official-both-notes', candidate:candidate(envBoth), action:'notes', red:true },
 { id:'official-url-notes', candidate:candidate({ANTHROPIC_BASE_URL:baseUrl}), action:'notes', red:true },
 { id:'official-token-notes', candidate:candidate({ANTHROPIC_AUTH_TOKEN:token}), action:'notes', red:true },
 { id:'pure-restore-notes', provider:{...record, settingsConfig:'{"env":{}}', meta:{opaqueFixtureSentinel:{retained:true}}}, action:'notes' },
 { id:'ordinary-both-notes', candidate:{...candidate(envBoth), normalizedCategory:'custom', category:'custom'}, action:'notes' },
 { id:'official-cancel', candidate:candidate(envBoth), action:'cancel' },
 { id:'claude-meta-notes', provider:{...record, sourceProviderId:'claude:synthetic-origin', meta:{
  opaqueFixtureSentinel:{retained:true}, providerType:'anthropic', apiFormat:'anthropic',
  claudeDesktopModelRoutes:{'claude-sonnet-5':{model:'claude-sonnet-5',labelOverride:'Synthetic Sonnet',supports1m:true,tierAlias:'sonnet'}},
  customHeaders:[{op:'set',name:'X-Synthetic-Fixture',value:'true',from:'',to:''}], modelRewrites:[{from:'old-synthetic',to:'new-synthetic'}],
 }}, action:'notes', red:true },
 { id:'claude-preset-token-notes', provider:{...record, settingsConfig:JSON.stringify({env:{ANTHROPIC_AUTH_TOKEN:token}}), sourceProviderId:'claude:synthetic-preset',meta:{gatewayProfile:{tool:'claude',profileId:'deepseek',endpointId:'anthropic'},apiFormat:'anthropic',opaqueFixtureSentinel:{retained:true}}}, action:'notes',red:true,preset:true },
 { id:'official-bom-notes', candidate:candidate({ANTHROPIC_AUTH_TOKEN:'\uFEFF'}), action:'notes', red:true },
 { id:'changed-url-missing-key', candidate:candidate({ANTHROPIC_BASE_URL:baseUrl}), action:'invalid', changes:{baseUrl:baseUrl+'/changed'}, greenOnly:true },
 { id:'changed-token-missing-url', candidate:candidate({ANTHROPIC_AUTH_TOKEN:token}), action:'invalid', changes:{apiKey:token+'-changed'}, greenOnly:true },
 { id:'custom-url-missing-key', candidate:{...candidate({ANTHROPIC_BASE_URL:baseUrl}),normalizedCategory:'custom',category:'custom'}, action:'invalid', greenOnly:true },
 { id:'new-custom-bom-key', create:true, action:'invalid', changes:{name:'Synthetic new',baseUrl,apiKey:'\uFEFF'}, greenOnly:true },
 { id:'copy-token-missing-url', candidate:candidate({ANTHROPIC_AUTH_TOKEN:token}), copy:true, action:'invalid', greenOnly:true },
 { id:'same-dialog-change-revert', candidate:candidate({ANTHROPIC_BASE_URL:baseUrl}), action:'revert', changes:{apiKey:' '}, revert:{apiKey:''}, greenOnly:true },
 { id:'changed-both-nel', candidate:candidate(envBoth), action:'invalid', changes:{baseUrl:'\u0085',apiKey:'\u0085'}, greenOnly:true },
 { id:'url-only-added-space-key', candidate:candidate({ANTHROPIC_BASE_URL:baseUrl}), action:'invalid', changes:{apiKey:' '}, greenOnly:true },
];
export const editedNote='Synthetic edited note';
export const counts={red:scenarios.filter(x=>!x.greenOnly).length,green:scenarios.length};
