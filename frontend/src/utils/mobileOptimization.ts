export interface MobileViewConfig {
  isMobile: boolean;
  isTablet: boolean;
  screenWidth: number;
  screenHeight: number;
  touchEnabled: boolean;
  orientation: 'portrait' | 'landscape';
}

export const BREAKPOINTS = {
  mobile: 480,
  tablet: 768,
  desktop: 1024,
  wide: 1440,
} as const;

export const getMobileViewConfig = (): MobileViewConfig => {
  const screenWidth = window.innerWidth;
  const screenHeight = window.innerHeight;
  const isMobile = screenWidth <= BREAKPOINTS.mobile;
  const isTablet = screenWidth > BREAKPOINTS.mobile && screenWidth <= BREAKPOINTS.tablet;
  const touchEnabled = 'ontouchstart' in window || navigator.maxTouchPoints > 0;
  const orientation = screenHeight > screenWidth ? 'portrait' : 'landscape';

  return {
    isMobile,
    isTablet,
    screenWidth,
    screenHeight,
    touchEnabled,
    orientation,
  };
};

export const useMobileOptimization = () => {
  const config = getMobileViewConfig();

  return {
    ...config,
    isSmallScreen: config.screenWidth < BREAKPOINTS.tablet,
    isLargeScreen: config.screenWidth >= BREAKPOINTS.desktop,
  };
};

export const getResponsiveGridCols = (baseColumns: number): number => {
  const config = getMobileViewConfig();

  if (config.isMobile) return 1;
  if (config.isTablet) return Math.max(2, Math.floor(baseColumns / 2));
  return baseColumns;
};

export const getTouchOptimizedSize = (baseSize: number): number => {
  const config = getMobileViewConfig();
  const minTouchTarget = 44;

  if (!config.touchEnabled) return baseSize;
  return Math.max(baseSize, minTouchTarget);
};

export const optimizePerformance = {
  enableLazyLoading: (element: HTMLElement) => {
    if ('IntersectionObserver' in window) {
      const observer = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
          if (entry.isIntersecting) {
            entry.target.classList.add('loaded');
            observer.unobserve(entry.target);
          }
        });
      });
      observer.observe(element);
    }
  },

  debounceResize: (callback: () => void, delay: number = 250): (() => void) => {
    let timeoutId: NodeJS.Timeout;
    return () => {
      clearTimeout(timeoutId);
      timeoutId = setTimeout(callback, delay);
    };
  },

  throttleScroll: (callback: () => void, limit: number = 100): (() => void) => {
    let inThrottle: boolean;
    return () => {
      if (!inThrottle) {
        callback();
        inThrottle = true;
        setTimeout(() => (inThrottle = false), limit);
      }
    };
  },

  preloadImage: (src: string): Promise<void> => {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve();
      img.onerror = reject;
      img.src = src;
    });
  },

  compressImage: (file: File, quality: number = 0.8): Promise<Blob> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = (event) => {
        const img = new Image();
        img.onload = () => {
          const canvas = document.createElement('canvas');
          canvas.width = img.width;
          canvas.height = img.height;
          const ctx = canvas.getContext('2d');
          if (ctx) {
            ctx.drawImage(img, 0, 0);
            canvas.toBlob((blob) => {
              if (blob) resolve(blob);
            }, 'image/jpeg', quality);
          }
        };
        img.onerror = reject;
        img.src = event.target?.result as string;
      };
      reader.onerror = reject;
      reader.readAsDataURL(file);
    });
  },
};

export const PWASupport = {
  isInstallable: (): boolean => {
    return 'serviceWorker' in navigator && 'PushManager' in window;
  },

  registerServiceWorker: async (swPath: string = '/service-worker.js'): Promise<void> => {
    if ('serviceWorker' in navigator) {
      try {
        await navigator.serviceWorker.register(swPath);
        console.log('Service Worker registered successfully');
      } catch (error) {
        console.error('Service Worker registration failed:', error);
      }
    }
  },

  cacheAssets: async (cacheName: string, urls: string[]): Promise<void> => {
    if ('caches' in window) {
      try {
        const cache = await caches.open(cacheName);
        await cache.addAll(urls);
        console.log('Assets cached successfully');
      } catch (error) {
        console.error('Asset caching failed:', error);
      }
    }
  },

  enableOfflineMode: async (): Promise<void> => {
    if ('serviceWorker' in navigator) {
      try {
        const registration = await navigator.serviceWorker.ready;
        console.log('Service Worker is active - offline mode enabled');
      } catch (error) {
        console.error('Offline mode setup failed:', error);
      }
    }
  },
};

export const MobileGestureSupport = {
  onSwipeLeft: (element: HTMLElement, callback: () => void): void => {
    let startX = 0;
    element.addEventListener('touchstart', (e) => {
      startX = e.touches[0].clientX;
    });
    element.addEventListener('touchend', (e) => {
      const endX = e.changedTouches[0].clientX;
      if (startX - endX > 50) callback();
    });
  },

  onSwipeRight: (element: HTMLElement, callback: () => void): void => {
    let startX = 0;
    element.addEventListener('touchstart', (e) => {
      startX = e.touches[0].clientX;
    });
    element.addEventListener('touchend', (e) => {
      const endX = e.changedTouches[0].clientX;
      if (endX - startX > 50) callback();
    });
  },

  onLongPress: (element: HTMLElement, callback: () => void, duration: number = 500): void => {
    let timeoutId: NodeJS.Timeout;
    element.addEventListener('touchstart', () => {
      timeoutId = setTimeout(callback, duration);
    });
    element.addEventListener('touchend', () => {
      clearTimeout(timeoutId);
    });
  },

  onDoubleTap: (element: HTMLElement, callback: () => void): void => {
    let lastTap = 0;
    element.addEventListener('touchend', (e) => {
      const currentTime = new Date().getTime();
      const tapLength = currentTime - lastTap;
      if (tapLength < 300 && tapLength > 0) {
        callback();
      }
      lastTap = currentTime;
    });
  },
};

export const ViewportOptimization = {
  getViewportHeight: (): number => {
    return Math.max(
      document.documentElement.clientHeight,
      window.innerHeight || 0
    );
  },

  getViewportWidth: (): number => {
    return Math.max(
      document.documentElement.clientWidth,
      window.innerWidth || 0
    );
  },

  lockViewport: (): void => {
    document.addEventListener('touchmove', (e) => {
      if (e.target instanceof HTMLElement && !e.target.closest('.scrollable')) {
        e.preventDefault();
      }
    }, { passive: false });
  },

  setMetaViewport: (scale: string = '1.0'): void => {
    const viewport = document.querySelector('meta[name="viewport"]');
    if (viewport) {
      viewport.setAttribute('content', `width=device-width, initial-scale=${scale}, maximum-scale=${scale}, user-scalable=no`);
    }
  },
};

export const AccessibilityOptimization = {
  enhanceContrast: (): void => {
    document.body.style.filter = 'contrast(1.2)';
  },

  increaseFontSize: (factor: number = 1.2): void => {
    document.documentElement.style.fontSize = `${16 * factor}px`;
  },

  enableReducedMotion: (): void => {
    document.documentElement.style.setProperty('--transition-duration', '0s');
  },

  setFocusVisibility: (): void => {
    const style = document.createElement('style');
    style.textContent = `
      *:focus-visible {
        outline: 3px solid #00d9ff;
        outline-offset: 2px;
      }
    `;
    document.head.appendChild(style);
  },
};

export const BatteryOptimization = {
  checkBatteryStatus: async (): Promise<any> => {
    if ('getBattery' in navigator) {
      return (navigator as any).getBattery();
    }
    return null;
  },

  reducePowerConsumption: (): void => {
    document.querySelectorAll('[data-animation]').forEach(el => {
      (el as HTMLElement).style.animationPlayState = 'paused';
    });
  },

  disableBackgroundSync: (): void => {
    if ('serviceWorker' in navigator) {
      navigator.serviceWorker.controller?.postMessage({
        type: 'DISABLE_BACKGROUND_SYNC',
      });
    }
  },
};
