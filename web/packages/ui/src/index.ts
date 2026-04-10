// Hearth Design System — @hearth/ui
//
// CSS side-effect imports (consumers should import these in their entry point):
//   import "@hearth/ui/tokens.css";
//   import "@hearth/ui/globals.css";

// ── Primitives ────────────────────────────────────────────────────────────
export { Badge } from "./components/Badge";
export type { BadgeProps, BadgeVariant } from "./components/Badge";

export { Button } from "./components/Button";
export type { ButtonProps } from "./components/Button";

export { Card } from "./components/Card";
export type { CardProps } from "./components/Card";

export { Avatar } from "./components/Avatar";
export type { AvatarProps } from "./components/Avatar";

export { Kbd } from "./components/Kbd";
export type { KbdProps } from "./components/Kbd";

export { Tooltip } from "./components/Tooltip";
export type { TooltipProps, TooltipSide } from "./components/Tooltip";

export { StatusChip } from "./components/StatusChip";
export type { StatusChipProps, StatusValue, StatusTone } from "./components/StatusChip";

export { Callout } from "./components/Callout";
export type { CalloutProps, CalloutVariant } from "./components/Callout";

export { ProgressBar } from "./components/ProgressBar";
export type { ProgressBarProps } from "./components/ProgressBar";

// ── Inputs ────────────────────────────────────────────────────────────────
export { SearchInput } from "./components/SearchInput";
export type { SearchInputProps } from "./components/SearchInput";

export { TextInput } from "./components/TextInput";
export type { TextInputProps } from "./components/TextInput";

export { Select } from "./components/Select";
export type { SelectProps, SelectOption } from "./components/Select";

export { FilterPills } from "./components/FilterPills";
export type { FilterPillsProps } from "./components/FilterPills";

export { SegmentedControl } from "./components/SegmentedControl";
export type { SegmentedControlProps, SegmentOption } from "./components/SegmentedControl";

export { KeyValueEditor } from "./components/KeyValueEditor";
export type { KeyValueEditorProps } from "./components/KeyValueEditor";

// ── Data display ──────────────────────────────────────────────────────────
export { DataTable } from "./components/DataTable";
export type { DataTableProps, TableDensity } from "./components/DataTable";

export { DescriptionList } from "./components/DescriptionList";
export type { DescriptionListProps, DescriptionListItem } from "./components/DescriptionList";

export { Timeline } from "./components/Timeline";
export type { TimelineProps, TimelineEvent, TimelineTone } from "./components/Timeline";

export { MetricTile } from "./components/MetricTile";
export type { MetricTileProps, MetricTone, MetricDelta } from "./components/MetricTile";

/** @deprecated Use `MetricTile`. */
export { StatCard } from "./components/StatCard";
export type { StatCardProps } from "./components/StatCard";

// ── Layout ────────────────────────────────────────────────────────────────
export { Sidebar } from "./components/Sidebar";
export type { SidebarProps, SidebarItem, SidebarGroup } from "./components/Sidebar";

export { PageContainer } from "./components/PageContainer";
export type { PageContainerProps, PageSize } from "./components/PageContainer";

export { PageHeader } from "./components/PageHeader";
export type { PageHeaderProps } from "./components/PageHeader";

export { Breadcrumbs } from "./components/Breadcrumbs";
export type { BreadcrumbsProps, BreadcrumbItem } from "./components/Breadcrumbs";

export { Tabs } from "./components/Tabs";
export type { TabsProps, Tab } from "./components/Tabs";

export { EmptyState } from "./components/EmptyState";
export type { EmptyStateProps } from "./components/EmptyState";

// ── Overlays ──────────────────────────────────────────────────────────────
export { Sheet } from "./components/Sheet";
export type { SheetProps, SheetSize, SheetSide } from "./components/Sheet";

export { Modal } from "./components/Modal";
export type { ModalProps, ModalSize } from "./components/Modal";

export { ConfirmDialog } from "./components/ConfirmDialog";
export type { ConfirmDialogProps } from "./components/ConfirmDialog";

export { CommandPalette } from "./components/CommandPalette";
export type { CommandPaletteProps, CommandItem } from "./components/CommandPalette";

export { ToastContainer } from "./components/Toast";
export type { ToastContainerProps } from "./components/Toast";

// ── Feedback ──────────────────────────────────────────────────────────────
export { Skeleton, SkeletonText, SkeletonCard, SkeletonTable } from "./components/Skeleton";
export type { SkeletonProps } from "./components/Skeleton";

// ── Hooks ─────────────────────────────────────────────────────────────────
export { useToast } from "./hooks/useToast";
export type { ToastItem } from "./hooks/useToast";
