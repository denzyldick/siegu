# THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!!

# Copyright 2020-2023 Tauri Programme within The Commons Conservancy
# SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: MIT

-keep class io.denzyl.siegu.* {
  native <methods>;
}

-keep class io.denzyl.siegu.WryActivity {
  public <init>(...);

  void setWebView(io.denzyl.siegu.RustWebView);
  java.lang.Class getAppClass(...);
  int getId();
  java.lang.String getVersion();
  int startActivity(...);
}

-keep class io.denzyl.siegu.Ipc {
  public <init>(...);

  @android.webkit.JavascriptInterface public <methods>;
}

-keep class io.denzyl.siegu.RustWebView {
  public <init>(...);

  void loadUrlMainThread(...);
  void loadHTMLMainThread(...);
  void evalScript(...);
}

-keep class io.denzyl.siegu.RustWebChromeClient,io.denzyl.siegu.RustWebViewClient {
  public <init>(...);
}
