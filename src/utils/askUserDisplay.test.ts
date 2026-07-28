import { describe, expect, it } from 'vitest';
import { extractAskUserQuestion, parseAskUserArguments } from './askUserDisplay';

describe('askUserDisplay', () => {
  it('extracts the final question from long markdown content', () => {
    const raw = [
      '## 通过 Homebrew Cask 发布 macOS App',
      '',
      '```ruby',
      'cask "buddy" do',
      'end',
      '```',
      '',
      '你打算用哪种方式？或者需要我帮你生成一个完整的 Cask 文件示例？',
    ].join('\n');

    expect(extractAskUserQuestion(raw)).toBe(
      '你打算用哪种方式？或者需要我帮你生成一个完整的 Cask 文件示例？',
    );
  });

  it('normalizes snake_case ask_user options', () => {
    const display = parseAskUserArguments(JSON.stringify({
      header: '发布方式',
      question: '请选择发布方式？',
      multi_select: true,
      options: [
        {
          label: '生成 Cask',
          description: '生成完整示例',
          requires_input: true,
          input_placeholder: '输入包名',
        },
      ],
    }));

    expect(display).toEqual({
      header: '发布方式',
      question: '请选择发布方式？',
      multiSelect: true,
      options: [
        {
          label: '生成 Cask',
          description: '生成完整示例',
          requiresInput: true,
          inputPlaceholder: '输入包名',
        },
      ],
    });
  });
});
