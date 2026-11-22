export interface TableRow {
  id: string;
}
export interface TreeItem {
  id: string;
  label: string;
  children?: TreeItem[];
}
export interface TableColumn<Row extends TableRow> {
  key: keyof Row;
  label: string;
  render?(value: Row[keyof Row], row: Row): Node | string;
}

export class RBTable<Row extends TableRow = TableRow> extends HTMLElement {
  rows: Row[];
  columns: TableColumn<Row>[];
  selectId(id: string): void;
}
export class RBTree extends HTMLElement {
  items: TreeItem[];
}
export class RBSplit extends HTMLElement {}
declare global {
  interface HTMLElementTagNameMap {
    'rb-table': RBTable;
    'rb-tree': RBTree;
    'rb-split': RBSplit;
  }
}
