import { useState } from "react";

import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";

import { xiaohan } from "../lib/xiaohan";



type Props = {

  personaId: string;

  onSuccess: () => void | Promise<void>;

  setFeedback: (f: SettingsFeedback | null) => void;

};



/** 从 Wiki 参考文本调用思考模型，手动重新生成结构化性格资料 */

export function PersonaRegenerateButton({ personaId, onSuccess, setFeedback }: Props) {

  const [running, setRunning] = useState(false);



  const run = async () => {

    setRunning(true);

    setFeedback(null);

    try {

      const result = await xiaohan.personaRegenerateProfile(personaId);

      setFeedback(successFeedback(result.message));

      await onSuccess();

    } catch (e) {

      setFeedback(parseApiError(e, "AI 更新性格"));

    } finally {

      setRunning(false);

    }

  };



  return (

    <button

      type="button"

      className="btn-secondary btn-sm"

      disabled={running}

      onClick={() => void run()}

      title="从 Wiki 参考文本手动生成简介/介绍/性格/说话风格（需配置思考模型，约 1–2 分钟）"

    >

      {running ? "AI 生成中…" : "AI 更新性格"}

    </button>

  );

}



