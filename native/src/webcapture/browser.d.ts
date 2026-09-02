/**
 * The slice of the WebExtensions API the Relay extension actually uses.
 *
 * Declared locally rather than pulled in as a dependency: the surface is
 * eight calls, and a types package would be a new dependency in the desktop
 * app's tree purely to describe code that never runs inside it.
 */

declare namespace chrome {
  namespace runtime {
    const lastError: { message?: string } | undefined;
    function getURL(path: string): string;
    const onInstalled: { addListener(cb: () => void): void };
    const onMessage: {
      addListener(
        cb: (
          message: unknown,
          sender: unknown,
          sendResponse: (response: unknown) => void,
        ) => boolean | void,
      ): void;
    };
  }

  namespace storage {
    interface Area {
      get(keys: string[] | null): Promise<Record<string, unknown>>;
      set(items: Record<string, unknown>): Promise<void>;
    }
    const local: Area;
    const sync: Area;
  }

  namespace tabs {
    interface Tab {
      id?: number;
      url?: string;
      title?: string;
    }
    function query(query: { active: boolean; currentWindow: boolean }): Promise<Tab[]>;
  }

  namespace scripting {
    interface InjectionResult<T> {
      result?: T;
      frameId: number;
    }
    function executeScript<T>(injection: {
      target: { tabId: number };
      files?: string[];
      func?: (...args: never[]) => T;
      world?: 'ISOLATED' | 'MAIN';
    }): Promise<InjectionResult<T>[]>;
  }

  namespace action {
    const onClicked: { addListener(cb: (tab: tabs.Tab) => void): void };
    function setBadgeText(details: { text: string; tabId?: number }): Promise<void>;
    function setBadgeBackgroundColor(details: {
      color: string;
      tabId?: number;
    }): Promise<void>;
    function setTitle(details: { title: string; tabId?: number }): Promise<void>;
  }

  namespace commands {
    const onCommand: { addListener(cb: (command: string, tab?: tabs.Tab) => void): void };
  }

  namespace notifications {
    function create(options: {
      type: 'basic';
      iconUrl: string;
      title: string;
      message: string;
    }): Promise<string>;
  }
}
