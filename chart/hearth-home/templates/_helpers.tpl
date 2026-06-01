{{/*
Expand the name of the chart.
*/}}
{{- define "hearth-home.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "hearth-home.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Common labels.
*/}}
{{- define "hearth-home.labels" -}}
helm.sh/chart: {{ include "hearth-home.name" . }}-{{ .Chart.Version | replace "+" "_" }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}

{{/*
Selector labels for a specific component.
Usage: {{ include "hearth-home.selectorLabels" (dict "context" . "component" "api") }}
*/}}
{{- define "hearth-home.selectorLabels" -}}
app.kubernetes.io/name: {{ include "hearth-home.name" .context }}
app.kubernetes.io/instance: {{ .context.Release.Name }}
app.kubernetes.io/component: {{ .component }}
{{- end }}

{{/*
Component labels (common + selector).
Usage: {{ include "hearth-home.componentLabels" (dict "context" . "component" "api") }}
*/}}
{{- define "hearth-home.componentLabels" -}}
{{ include "hearth-home.labels" .context }}
{{ include "hearth-home.selectorLabels" (dict "context" .context "component" .component) }}
{{- end }}

{{/*
Database URL — assembles from Bitnami subchart or external config.
The password placeholder $(DB_PASSWORD) is expanded at runtime by Kubernetes
when used alongside a valueFrom env var named DB_PASSWORD.
*/}}
{{- define "hearth-home.databaseUrl" -}}
{{- if .Values.postgresql.enabled -}}
postgres://{{ .Values.postgresql.auth.username }}:$(DB_PASSWORD)@{{ .Release.Name }}-postgresql:5432/{{ .Values.postgresql.auth.database }}
{{- else -}}
postgres://{{ .Values.externalDatabase.user }}:$(DB_PASSWORD)@{{ .Values.externalDatabase.host }}:{{ .Values.externalDatabase.port | default 5432 }}/{{ .Values.externalDatabase.database }}
{{- end -}}
{{- end }}

{{/*
Database secret name — resolves to Bitnami-generated secret or user-provided one.
*/}}
{{- define "hearth-home.databaseSecretName" -}}
{{- if .Values.externalDatabase.existingSecret -}}
{{- .Values.externalDatabase.existingSecret }}
{{- else if .Values.postgresql.enabled -}}
{{- .Release.Name }}-postgresql
{{- else -}}
{{- include "hearth-home.fullname" . }}-db
{{- end -}}
{{- end }}

{{/*
Database secret key — the key within the Secret that holds the password.
*/}}
{{- define "hearth-home.databaseSecretKey" -}}
{{- if .Values.externalDatabase.existingSecretPasswordKey -}}
{{- .Values.externalDatabase.existingSecretPasswordKey }}
{{- else if .Values.postgresql.enabled -}}
password
{{- else -}}
password
{{- end -}}
{{- end }}

{{/*
Internal service URLs for cross-service communication.
*/}}
{{- define "hearth-home.apiUrl" -}}
http://{{ include "hearth-home.fullname" . }}-api:{{ .Values.api.service.port }}
{{- end }}

{{- define "hearth-home.atticUrl" -}}
http://{{ include "hearth-home.fullname" . }}-attic:{{ .Values.attic.service.port }}
{{- end }}

{{- define "hearth-home.kanidmUrl" -}}
https://{{ include "hearth-home.fullname" . }}-kanidm:{{ .Values.kanidm.service.port }}
{{- end }}

{{- define "hearth-home.headscaleUrl" -}}
http://{{ include "hearth-home.fullname" . }}-headscale:{{ .Values.headscale.service.port }}
{{- end }}

{{- define "hearth-home.synapseUrl" -}}
http://{{ include "hearth-home.fullname" . }}-synapse:{{ .Values.synapse.service.port }}
{{- end }}

{{- define "hearth-home.nextcloudUrl" -}}
http://{{ include "hearth-home.fullname" . }}-nextcloud:{{ .Values.nextcloud.service.port }}
{{- end }}

{{- define "hearth-home.stalwartUrl" -}}
http://{{ include "hearth-home.fullname" . }}-stalwart:{{ .Values.stalwart.service.port }}
{{- end }}

{{/*
Secrets helpers — resolve inline vs existing secret references.
*/}}
{{- define "hearth-home.secretName" -}}
{{- if .Values.secrets.existingSecret -}}
{{- .Values.secrets.existingSecret }}
{{- else -}}
{{- include "hearth-home.fullname" . }}-secrets
{{- end -}}
{{- end }}

{{/*
Image pull secrets.
*/}}
{{- define "hearth-home.imagePullSecrets" -}}
{{- with .Values.global.imagePullSecrets }}
imagePullSecrets:
  {{- toYaml . | nindent 2 }}
{{- end }}
{{- end }}

{{/*
Hardened container securityContext — PSS restricted compliant.
Use on every container that supports it. Components that need write paths
must mount emptyDir volumes for those paths (see oauth2-proxy / api /
build-worker — they only need /tmp). Components that legitimately need
root or RW root (nextcloud apache, kanidm before TLS rework) must opt
out and document the accepted risk.

Usage:
  securityContext:
    {{- include "hearth-home.restrictedSecurityContext" . | nindent 12 }}
*/}}
{{- define "hearth-home.restrictedSecurityContext" -}}
allowPrivilegeEscalation: false
readOnlyRootFilesystem: true
runAsNonRoot: true
capabilities:
  drop: [ALL]
seccompProfile:
  type: RuntimeDefault
{{- end -}}

{{/*
cert-manager issuer reference. Resolves to the right ClusterIssuer or Issuer
depending on the certManager.issuer.type setting. Only valid when
.Values.certManager.enabled is true.
*/}}
{{- define "hearth-home.certIssuerRef" -}}
{{- if eq .Values.certManager.issuer.type "existing" -}}
name: {{ required "certManager.issuer.existing.name is required when issuer.type=existing" .Values.certManager.issuer.existing.name }}
kind: {{ .Values.certManager.issuer.existing.kind }}
group: cert-manager.io
{{- else -}}
name: {{ include "hearth-home.fullname" . }}-issuer
kind: ClusterIssuer
group: cert-manager.io
{{- end -}}
{{- end }}

{{/*
cert-manager Ingress annotation. Returns the `cert-manager.io/cluster-issuer`
or `cert-manager.io/issuer` annotation line keyed to the configured issuer,
so the ingress-shim controller will issue + auto-renew a cert for the host.

Emits nothing when:
  - certManager is disabled (passthrough — no auto-wiring), OR
  - the user has supplied their own `*.ingress.tls` list (BYO certs win).

Usage (inside metadata.annotations):
  {{- include "hearth-home.certManagerIngressAnnotation" (dict "context" . "userTls" .Values.api.ingress.tls) | nindent 4 }}
*/}}
{{- define "hearth-home.certManagerIngressAnnotation" -}}
{{- if and .context.Values.certManager.enabled (not .userTls) -}}
{{- if eq .context.Values.certManager.issuer.type "existing" -}}
{{- if eq .context.Values.certManager.issuer.existing.kind "ClusterIssuer" -}}
cert-manager.io/cluster-issuer: {{ required "certManager.issuer.existing.name is required when issuer.type=existing" .context.Values.certManager.issuer.existing.name | quote }}
{{- else -}}
cert-manager.io/issuer: {{ required "certManager.issuer.existing.name is required when issuer.type=existing" .context.Values.certManager.issuer.existing.name | quote }}
{{- end -}}
{{- else -}}
cert-manager.io/cluster-issuer: {{ printf "%s-issuer" (include "hearth-home.fullname" .context) | quote }}
{{- end -}}
{{- end -}}
{{- end -}}

{{/*
Ingress `spec.tls` block. The precedence is:

  1. User-supplied `*.ingress.tls` always wins — emit verbatim. This
     preserves the existing BYO-cert and wildcard-cert deployment paths
     where the operator already manages secrets out of band.
  2. Else if certManager is enabled, auto-populate one tls entry per
     ingress host using a deterministic secret name
     (`<release-fullname>-<component>-tls`). Pair with
     `certManagerIngressAnnotation` so the ingress-shim controller
     issues the cert.
  3. Else emit nothing — same as today.

Usage (inside spec):
  {{- include "hearth-home.ingressTls" (dict "context" . "userTls" .Values.api.ingress.tls "hosts" .Values.api.ingress.hosts "component" "api") | nindent 2 }}
*/}}
{{- define "hearth-home.ingressTls" -}}
{{- if .userTls -}}
tls:
{{ toYaml .userTls }}
{{- else if .context.Values.certManager.enabled -}}
tls:
{{- $context := .context }}
{{- $component := .component }}
{{- range .hosts }}
- hosts:
    - {{ .host | quote }}
  secretName: {{ printf "%s-%s-tls" (include "hearth-home.fullname" $context) $component | quote }}
{{- end }}
{{- end -}}
{{- end -}}
