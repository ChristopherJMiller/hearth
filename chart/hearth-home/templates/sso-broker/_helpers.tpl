{{/*
SSO Broker issuer URL. Resolution order:
  1. .Values.ssoBroker.issuerUrl, if explicitly set.
  2. Derived as `https://<host>/oidc`, where <host> is the first entry of
     .Values.api.ingress.hosts (requires api.ingress.enabled).

This helper is only ever included from templates already gated on
capabilities.ssoBroker (Kanidm bootstrap, signing-key Secret, hearth-api env,
Synapse OIDC issuer — see docs/sso-broker.md), so a hard `fail` here when
neither source is available is appropriate rather than silently emitting an
unusable URL.

Usage:
  {{ include "hearth-home.ssoBroker.issuerUrl" . }}
*/}}
{{- define "hearth-home.ssoBroker.issuerUrl" -}}
{{- if .Values.ssoBroker.issuerUrl -}}
{{- .Values.ssoBroker.issuerUrl -}}
{{- else if and .Values.api.ingress.enabled .Values.api.ingress.hosts -}}
{{- printf "https://%s/oidc" (index .Values.api.ingress.hosts 0).host -}}
{{- else -}}
{{- fail "ssoBroker.issuerUrl is not set and no api.ingress.hosts entry is available to derive it from. Set ssoBroker.issuerUrl explicitly, or enable api.ingress with at least one host." -}}
{{- end -}}
{{- end }}
