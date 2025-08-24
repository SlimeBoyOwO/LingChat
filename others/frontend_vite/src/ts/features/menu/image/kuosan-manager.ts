import { DOM } from "../../../ui/dom.js";

export class KousanManager {
  private kuosanValue: string;

  constructor() {
    this.kuosanValue = "10"; // 默认光晕扩散值
    this.init();
  }

  private init() {
    this.initKuosan();
    this.bindEvents();
  }

  private initKuosan() {
    const savedKuosan = localStorage.getItem("Kuosan") || this.kuosanValue;
    if (DOM.image.filterKuosan) DOM.image.filterKuosan.value = savedKuosan;
    this.kuosanValue = savedKuosan;

    this.applyKuosan();
  }

  private bindEvents() {
    // 光晕扩散设置变化事件
    DOM.image.filterKuosan?.addEventListener("change", (e) => {
      if (!(e.target instanceof HTMLInputElement)) return;

      this.kuosanValue = e.target.value;
      this.saveKuosanSetting();
      this.applyKuosan();
    });
  }

  private applyKuosan() {
    const filterValue = `drop-shadow(0 0 ${this.kuosanValue}px rgba(255, 255, 255, 1))`;

    if (DOM.image.qinling) DOM.image.qinling.style.filter = filterValue;
    if (DOM.image.qinlingtest) DOM.image.qinlingtest.style.filter = filterValue;

    if (DOM.image.kuosanPreview && DOM.image.kousanPreviewImg) {
      DOM.image.kousanPreviewImg.style.filter = `brightness(1.1) saturate(1.25) drop-shadow(0 0 ${this.kuosanValue}px rgba(255, 255, 255, 0.8))`;
      DOM.image.kousanPreviewImg.style.transition = "filter 0.3s ease-in-out";

      localStorage.setItem("KuosanValue", this.kuosanValue);
    }
  }

  private saveKuosanSetting() {
    localStorage.setItem("Kuosan", this.kuosanValue);
  }

  public updatePreviewBackground(url: string) {
    DOM.image.kuosanPreview.style.backgroundImage = `url(${url})`;
    DOM.image.kuosanPreview.style.backgroundSize = "cover";
    DOM.image.kuosanPreview.style.backgroundPosition = "center";
    DOM.image.kuosanPreview.style.borderRadius = "8px";
    DOM.image.kuosanPreview.style.boxShadow = "0 4px 8px rgba(0, 0, 0, 0.2)";
  }
}
