export function onCommittedInput(
  input: HTMLInputElement | HTMLTextAreaElement,
  changed: (value: string) => void
): () => void;
export class RBField extends HTMLElement {}
export class RBMenu extends HTMLElement {
  open(target?: HTMLElement): void;
  close(focus?: boolean): void;
}
export class RBDialog extends HTMLElement {
  open(): void;
  close(): void;
}
