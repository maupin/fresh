/// <reference path="./lib/fresh.d.ts" />

const editor = getEditor();

const GIT_BRANCH = "branch";

let lastDetectedTimestamp = 0;
let lastDetectedBranch = "Detecting branch ...";

async function getCurrentGitBranch(): Promise<string> {
  const now = Date.now();

  if (now - lastDetectedTimestamp < 5000) {
    return lastDetectedBranch;
  }

  const cwd = editor.getCwd();
  const result = await editor.spawnProcess(
    "git",
    ["rev-parse", "--abbrev-ref", "HEAD"],
    cwd,
  );

  if (result.exit_code === 0) {
    const branch = result.stdout.trim();

    lastDetectedBranch = branch || "HEAD";
  } else {
    lastDetectedBranch = "Not in git";
  }

  lastDetectedTimestamp = now;

  return lastDetectedBranch;
}

editor.registerStatusBarElement(GIT_BRANCH, "Git: branch");

[
  "buffer_activated",
  "buffer_deactivated",
  "buffer_closed",
  "after_file_open",
  "after_file_save",
  "after_insert",
  "after_delete",
  "cursor_moved",
  "render_start",
].forEach((event) => {
  editor.on(event, async () => {
    editor.setStatusBarElementValue(GIT_BRANCH, await getCurrentGitBranch());
  });
});
