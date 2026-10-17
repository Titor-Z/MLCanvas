/**
 * 统一图标集（见 artboards/ui-icons.html 规范）：
 * 全部 viewBox="0 0 16 16"，1.5 描边、圆头圆角，尺寸由外部 CSS 决定（16 默认 / 20 强调）。
 * 只有「停止」这类实心符号用填充。
 */
import type { ComponentProps } from "svelte";
import Icon from "./Icon.svelte";

export type IconName =
  | "select"
  | "hand"
  | "annotate"
  | "design"
  | "resource"
  | "market"
  | "board"
  | "chat"
  | "send"
  | "stop"
  | "plus"
  | "close"
  | "check"
  | "clock"
  | "chevron"
  | "caret-down"
  | "retry"
  | "copy"
  | "compact"
  | "doc"
  | "spark"
  | "spinner";

export { Icon };
export type IconProps = ComponentProps<Icon>;
