// Bundle every class icon and resolve them by the icon file name the backend sends.
const iconUrls = import.meta.glob("./assets/icons/*.png", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

export function iconUrlFor(iconFile: string | null): string | null {
  if (!iconFile) return null;
  return iconUrls[`./assets/icons/${iconFile}`] ?? null;
}
