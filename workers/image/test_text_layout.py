import unittest
from text_layout import lines, units
from test_render_frame import RenderFrameTest


class TextLayoutTest(unittest.TestCase):
    def test_windows_and_carriage_return_line_breaks_preserve_empty_lines(self):
        expected = ['字幕', '', '次の行', '']
        for separator in ['\n', '\r\n', '\r']:
            text = separator.join(expected)
            for wrap in [False, True]:
                self.assertEqual(lines(text, len, 40, wrap), expected)

    def test_line_ending_variants_render_identical_pixels_without_mutating_input(self):
        harness = RenderFrameTest()
        harness.setUpClass()
        for wrap in [False, True]:
            expected = None
            for separator in ['\n', '\r\n', '\r']:
                text = separator.join(['字幕', '', '次の行', ''])
                primitive = dict(type='text', text=text, font_size=24, color='#123456',
                                 layout=dict(alignment='center', wrap=wrap))
                image, report = harness.draw(harness.packet([
                    harness.layer(primitive, width=180, height=180)], width=200, height=200))
                self.assertEqual(primitive['text'], text)
                self.assertFalse(report['issues'])
                current = (image.tobytes(), report)
                if expected is None: expected = current
                self.assertEqual(current, expected)
                image.close()

    def test_katakana_runs_survive_wrapping_and_tail_balancing(self):
        from PIL import ImageFont
        harness = RenderFrameTest(); harness.setUpClass()
        font = ImageFont.truetype(str(harness.font), 64)
        for text, word in [
            ('音声ができたら、その長さに合わせてセリフを並べるわ。', 'セリフ'),
            ('口の動きと話しているキャラクターも確認するのだ。', 'キャラクター'),
            ('これはｷｬﾗｸﾀｰです。', 'ｷｬﾗｸﾀｰ'),
            ('音声のエンジンを確認する。', 'エンジン'),
        ]:
            for width in [640, 1000, 1280]:
                result = lines(text, font.getlength, width, True)
                self.assertEqual(''.join(result), text)
                self.assertTrue(any(word in row for row in result), result)
                self.assertTrue(all(font.getlength(row) <= width for row in result))
        self.assertEqual(lines('アイウエオ', len, 2, True), ['アイ', 'ウエ', 'オ'])
        self.assertEqual(lines('キャ\nラクター', len, 12, True), ['キャ', 'ラクター'])

    def test_halfwidth_voicing_marks_stay_with_their_base(self):
        self.assertEqual(units('ｶﾞﾊﾟｳﾞ'), ['ｶﾞ', 'ﾊﾟ', 'ｳﾞ'])
        text = 'ｶﾞｷﾞｸﾞﾊﾟﾋﾟﾌﾟ'
        result = lines(text, lambda s: len(units(s)), 2, True)
        self.assertEqual(''.join(result), text)
        self.assertTrue(all(not row.startswith(('ﾞ', 'ﾟ')) for row in result))
        self.assertEqual(result, ['ｶﾞｷﾞ', 'ｸﾞﾊﾟ', 'ﾋﾟﾌﾟ'])
        # An explicitly requested break is never joined or normalized.
        self.assertEqual(lines('ｶ\nﾞ', len, 20, True), ['ｶ', 'ﾞ'])

    def test_preserves_text_empty_lines_and_combining_sequences(self):
        for text in ['日本語の字幕。次の文です。', 'long English words here', 'a\u0301b', '👨\u200d👩\u200d👧\u200d👦🇯🇵']:
            result = lines(text, len, 5, True)
            self.assertEqual(''.join(result), text)
        self.assertEqual(lines('a\n\nb', len, 10, True), ['a', '', 'b'])
        self.assertEqual(units('a\u0301🇯🇵👩\u200d💻'), ['a\u0301', '🇯🇵', '👩\u200d💻'])
        self.assertEqual(lines('あいう、え', len, 3, True), ['あい', 'う、え'])
        self.assertEqual(lines('a verylongword', len, 6, True), ['a ', 'verylo', 'ngword'])

    def test_cjk_orphan_tail_balanced_without_losing_characters(self):
        text = 'ずんだもんなのだ。今日は二人で話すのだ。'
        result = lines(text, len, 19, True)
        self.assertEqual(''.join(result), text)
        self.assertEqual(len(result), 2)
        self.assertGreater(min(map(len, result)), 5)
        self.assertLessEqual(max(map(len, result)), 19)
        self.assertEqual(lines('あいうえおかきくけこ\nだ。', len, 19, True),
                         ['あいうえおかきくけこ', 'だ。'])
        self.assertEqual(lines(text, len, 19, False), [text])
        self.assertEqual(lines('台本から掛け合いを作れるようになったのだ。', len, 19, True),
                         ['台本から掛け合いを', '作れるようになったのだ。'])
        for text in ['日本語（字幕）を確認。', '日本語とOpenAIの字幕です。', '日本語👩\u200d💻🇯🇵の説明です。']:
            result = lines(text, len, 12, True)
            self.assertEqual(''.join(result), text)
            self.assertTrue(all(not row.startswith(('。', '）')) for row in result))
            self.assertTrue(all(not row.endswith('（') for row in result))

    def test_measured_alignment_wrapping_and_vertical_overflow(self):
        harness = RenderFrameTest()
        harness.setUpClass()
        p = dict(type='text', text='字幕の折り返しを確認します', font_size=24,
                 color='#000000', outline=dict(width=2, color='#000000'),
                 layout=dict(alignment='center', wrap=True))
        layer = harness.layer(p, x=10, y=5, width=100, height=180)
        image, report = harness.draw(harness.packet([layer], width=120, height=200))
        self.assertFalse(report['issues'])
        self.assertLess(image.crop((10, 35, 110, 180)).convert('L').getextrema()[0], 128)
        layer['transform']['height'] = 20
        _, report = harness.draw(harness.packet([layer], width=120, height=200))
        self.assertTrue(report['issues'])
        p['text'] = '字'; layer['transform']['height'] = 100
        for align in ['left', 'center', 'right']:
            p['layout']['alignment'] = align
            image, report = harness.draw(harness.packet([layer], width=120, height=200))
            from PIL import ImageChops, Image
            box = ImageChops.difference(image.convert('RGB'), Image.new('RGB',image.size,'white')).getbbox()
            if align == 'left': self.assertLessEqual(box[0], 12)
            if align == 'center': self.assertLessEqual(abs((box[0]+box[2])/2-60), 2)
            if align == 'right': self.assertGreaterEqual(box[2], 108)
            self.assertFalse(report['issues'])

    def test_build_title_wraps_in_wide_and_portrait_cards_without_truncation(self):
        harness = RenderFrameTest()
        harness.setUpClass()
        text = 'Irodori一括制作・模擬音声・irodori-build-fixture-20261010-02'
        for width, height, x, y, box_width, box_height in [
                (1920, 1080, 160, 150, 1600, 500),
                (1080, 1920, 80, 480, 920, 700)]:
            p = dict(type='text', text=text, font_size=64, color='#111111',
                     layout=dict(alignment='left', wrap=True))
            layer = harness.layer(p, x=x, y=y, width=box_width, height=box_height)
            _, report = harness.draw(harness.packet([layer], width=width, height=height))
            self.assertFalse(report['issues'])
            p['layout']['wrap'] = False
            _, report = harness.draw(harness.packet([layer], width=width, height=height))
            self.assertTrue(report['issues'])
        p['layout']['wrap'] = True
        p['text'] = text * 100
        _, report = harness.draw(harness.packet([layer], width=1080, height=1920))
        self.assertTrue(report['issues'])
