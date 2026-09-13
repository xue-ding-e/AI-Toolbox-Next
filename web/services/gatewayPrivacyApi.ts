import { invoke } from '@tauri-apps/api/core';

export interface GatewayPrivacyCustomRule {
  id: string;
  name: string;
  enabled: boolean;
  kind: 'literal' | 'regex';
  pattern: string;
  priority: number;
}

export interface GatewayPrivacyRules {
  builtins: string[];
  custom: GatewayPrivacyCustomRule[];
  allowlist: string[];
}

export interface GatewayPrivacySettings {
  enabled: boolean;
  rules: GatewayPrivacyRules;
}

export interface GatewayPrivacyDetail {
  matched_values: number;
  restored_values: number;
  rules: Record<string, number>;
  log_redacted: boolean;
  failed: boolean;
}

export interface GatewayPrivacyPreview {
  redacted: string;
  restored: string;
  detail: GatewayPrivacyDetail;
}

export const getGatewayPrivacySettings = () =>
  invoke<GatewayPrivacySettings>('proxy_gateway_get_privacy_settings');

export const updateGatewayPrivacySettings = (update: Partial<GatewayPrivacySettings>) =>
  invoke<GatewayPrivacySettings>('proxy_gateway_update_privacy_settings', { update });

export const previewGatewayPrivacy = (rules: GatewayPrivacyRules, text: string) =>
  invoke<GatewayPrivacyPreview>('proxy_gateway_preview_privacy', { rules, text });
