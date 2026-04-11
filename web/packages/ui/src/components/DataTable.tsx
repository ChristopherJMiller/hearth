import {
  type ColumnDef,
  type SortingState,
  type ColumnFiltersState,
  type Row,
  type RowSelectionState,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getExpandedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { type ReactNode, useState, useEffect, Fragment } from "react";

export type TableDensity = "comfortable" | "cozy";

export interface DataTableProps<T> {
  data: T[];
  columns: ColumnDef<T, unknown>[];
  onRowClick?: (row: T) => void;
  emptyMessage?: string;
  density?: TableDensity;
  selectable?: boolean;
  onSelectionChange?: (rows: T[]) => void;
  pageSize?: number;
  renderExpanded?: (row: T) => ReactNode;
  getRowId?: (row: T) => string;
  highlightRow?: (row: T) => boolean;
  className?: string;
}

const densityCell: Record<TableDensity, string> = {
  comfortable: "py-3.5 px-5",
  cozy: "py-2.5 px-4",
};

function SortIcon({ direction }: { direction: "asc" | "desc" | false }) {
  if (!direction) {
    return (
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="opacity-30">
        <path d="M7 15l5 5 5-5M7 9l5-5 5 5" />
      </svg>
    );
  }
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="opacity-90">
      {direction === "asc" ? <path d="M7 14l5-5 5 5" /> : <path d="M7 10l5 5 5-5" />}
    </svg>
  );
}

function Checkbox({
  checked,
  indeterminate,
  onChange,
  ariaLabel,
}: {
  checked: boolean;
  indeterminate?: boolean;
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  ariaLabel?: string;
}) {
  return (
    <input
      type="checkbox"
      checked={checked}
      aria-label={ariaLabel}
      ref={(el) => {
        if (el) el.indeterminate = !!indeterminate && !checked;
      }}
      onChange={onChange}
      onClick={(e) => e.stopPropagation()}
      className="w-4 h-4 rounded-[4px] border border-border bg-surface-sunken accent-ember cursor-pointer"
    />
  );
}

export function DataTable<T>({
  data,
  columns,
  onRowClick,
  emptyMessage = "No data found",
  density = "comfortable",
  selectable = false,
  onSelectionChange,
  pageSize,
  renderExpanded,
  getRowId,
  highlightRow,
  className = "",
}: DataTableProps<T>) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({});
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  // Build the final columns list with optional leading select + expand columns.
  const finalColumns: ColumnDef<T, unknown>[] = [];
  if (selectable) {
    finalColumns.push({
      id: "__select",
      header: ({ table }) => (
        <Checkbox
          checked={table.getIsAllPageRowsSelected()}
          indeterminate={table.getIsSomePageRowsSelected()}
          onChange={table.getToggleAllPageRowsSelectedHandler()}
          ariaLabel="Select all"
        />
      ),
      cell: ({ row }) => (
        <Checkbox
          checked={row.getIsSelected()}
          onChange={row.getToggleSelectedHandler()}
          ariaLabel="Select row"
        />
      ),
      enableSorting: false,
      size: 40,
    });
  }
  if (renderExpanded) {
    finalColumns.push({
      id: "__expand",
      header: () => null,
      cell: ({ row }) => (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            row.toggleExpanded();
          }}
          className="w-6 h-6 flex items-center justify-center text-text-tertiary hover:text-text-primary cursor-pointer"
          aria-label={row.getIsExpanded() ? "Collapse row" : "Expand row"}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
               className={`transition-transform ${row.getIsExpanded() ? "rotate-90" : ""}`}>
            <path d="M9 6l6 6-6 6" />
          </svg>
        </button>
      ),
      enableSorting: false,
      size: 32,
    });
  }
  finalColumns.push(...columns);

  const table = useReactTable({
    data,
    columns: finalColumns,
    state: { sorting, columnFilters, rowSelection, expanded },
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    onRowSelectionChange: setRowSelection,
    onExpandedChange: setExpanded as any,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getPaginationRowModel: pageSize ? getPaginationRowModel() : undefined,
    getExpandedRowModel: renderExpanded ? getExpandedRowModel() : undefined,
    getRowId: getRowId ? (row) => getRowId(row) : undefined,
    enableRowSelection: selectable,
    initialState: pageSize ? { pagination: { pageSize } } : undefined,
  });

  useEffect(() => {
    if (!selectable || !onSelectionChange) return;
    const rows = table.getSelectedRowModel().rows.map((r) => r.original);
    onSelectionChange(rows);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rowSelection]);

  const cellPad = densityCell[density];
  const rows = table.getRowModel().rows;

  return (
    <div className={`w-full flex flex-col gap-3 ${className}`}>
      <div className="w-full overflow-x-auto rounded-md border border-border-subtle bg-surface">
        <table className="w-full border-collapse text-sm">
          <thead>
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id} className="bg-surface-sunken">
                {headerGroup.headers.map((header) => (
                  <th
                    key={header.id}
                    className={`text-left font-semibold text-text-tertiary uppercase border-b border-border-subtle select-none text-2xs tracking-wide ${cellPad}`}
                    style={{
                      cursor: header.column.getCanSort() ? "pointer" : "default",
                      width: header.column.columnDef.size,
                    }}
                    onClick={header.column.getToggleSortingHandler()}
                  >
                    <div className="flex items-center gap-1.5">
                      {header.isPlaceholder
                        ? null
                        : flexRender(header.column.columnDef.header, header.getContext())}
                      {header.column.getCanSort() && (
                        <SortIcon direction={header.column.getIsSorted()} />
                      )}
                    </div>
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td
                  colSpan={finalColumns.length}
                  className="text-center text-text-tertiary text-sm py-12 px-4"
                >
                  {emptyMessage}
                </td>
              </tr>
            ) : (
              rows.map((row: Row<T>) => {
                const highlighted = highlightRow?.(row.original) ?? false;
                return (
                  <Fragment key={row.id}>
                    <tr
                      className={`border-b border-border-subtle last:border-b-0 transition-colors duration-100 ${
                        row.getIsSelected() ? "bg-ember-faint" : highlighted ? "bg-warning-faint" : ""
                      } ${
                        onRowClick
                          ? "cursor-pointer hover:bg-surface-raised"
                          : ""
                      }`}
                      onClick={() => onRowClick?.(row.original)}
                    >
                      {row.getVisibleCells().map((cell) => (
                        <td
                          key={cell.id}
                          className={`text-text-primary align-middle ${cellPad}`}
                        >
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </td>
                      ))}
                    </tr>
                    {renderExpanded && row.getIsExpanded() && (
                      <tr className="bg-surface-sunken">
                        <td
                          colSpan={finalColumns.length}
                          className={`border-b border-border-subtle ${cellPad}`}
                        >
                          {renderExpanded(row.original)}
                        </td>
                      </tr>
                    )}
                  </Fragment>
                );
              })
            )}
          </tbody>
        </table>
      </div>

      {pageSize && rows.length > 0 && (
        <div
          className="flex items-center justify-between text-text-tertiary text-xs"
         
        >
          <div>
            Showing {table.getRowModel().rows.length} of {table.getFilteredRowModel().rows.length}
          </div>
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => table.previousPage()}
              disabled={!table.getCanPreviousPage()}
              className="px-2.5 py-1.5 rounded-sm border border-border-subtle hover:bg-surface-raised hover:text-text-primary disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
            >
              Prev
            </button>
            <span className="px-2 tabular-nums">
              {table.getState().pagination.pageIndex + 1} / {table.getPageCount() || 1}
            </span>
            <button
              type="button"
              onClick={() => table.nextPage()}
              disabled={!table.getCanNextPage()}
              className="px-2.5 py-1.5 rounded-sm border border-border-subtle hover:bg-surface-raised hover:text-text-primary disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
