import type { ReactNode } from "react";



interface Props {

  title: string;

  description?: string;

  accent?: boolean;

  /** 子项贴边排列（如 settings-field 行），不加内边距容器 */

  flush?: boolean;

  children: ReactNode;

}



/** 设置分组：标题在外、内容在圆角卡片内 */

export function SettingsSection({

  title,

  description,

  accent = false,

  flush = false,

  children,

}: Props) {

  return (

    <section className="pref-group">

      <div className="pref-group__meta">

        <h3 className="pref-group__title">{title}</h3>

        {description && <p className="pref-group__desc">{description}</p>}

      </div>

      <div className={`pref-group__card${accent ? " pref-group__card--accent" : ""}`}>

        {flush ? children : <div className="pref-group__pad">{children}</div>}

      </div>

    </section>

  );

}


