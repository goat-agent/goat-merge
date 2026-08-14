import type { ReactNode } from "react";

import type { Asked } from "@/shared/lib";

import { Waiting } from "@/shared/ui";

import { WhatWentWrong } from "./WhatWentWrong";

export function WhatWeHave<T>({
  asked,
  of,
  children,
}: {
  asked: Asked<T>;
  of: string;
  children: (answer: T) => ReactNode;
}) {
  if (asked.answer !== null) {
    return (
      <>
        {asked.trouble ? (
          <WhatWentWrong trouble={asked.trouble} place="banner" onAgain={asked.again} />
        ) : null}
        {asked.refreshing && !asked.trouble ? <Waiting of={of} place="banner" /> : null}
        {children(asked.answer)}
      </>
    );
  }
  if (asked.trouble) {
    return <WhatWentWrong trouble={asked.trouble} onAgain={asked.again} />;
  }
  return <Waiting of={of} />;
}
