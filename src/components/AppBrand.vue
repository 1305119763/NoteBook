<template>
  <div class="nav-brand">
    <div class="nav-brand-visual">
      <img class="nav-brand-logo" src="/screen.png" width="42" height="42" alt="" decoding="async" />
      <div class="nav-brand-text">
        <div class="nav-brand-title">NoteBook</div>
        <div class="nav-brand-sub">本地笔记，安全不联网不泄露</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.nav-brand {
  padding: 0 6px 8px;
}
.nav-brand-visual {
  display: flex;
  align-items: center;
  gap: 10px;
}
.nav-brand-logo {
  width: 42px;
  height: 42px;
  object-fit: contain;
  flex-shrink: 0;
  border-radius: 10px;
  background: #000;
}
.nav-brand-text {
  min-width: 0;
  flex: 1;
}
.nav-brand-title {
  font-size: 20px;
  font-weight: 800;
  letter-spacing: -0.03em;
  color: var(--text);
  line-height: 1.15;
}
.nav-brand-sub {
  margin-top: 3px;
  font-size: 11px;
  font-weight: 500;
  line-height: 1.35;
  color: var(--muted);
}
</style>
