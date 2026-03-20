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
