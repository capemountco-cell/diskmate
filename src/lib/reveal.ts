/** Opens Finder with the item selected. The window is granted exactly this. */
export async function reveal(path: string): Promise<void> {
  try {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(path);
  } catch {
    // Refusing to reveal is not worth an error banner: the path is on screen.
  }
}
