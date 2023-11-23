using Avalonia;
using LittleBigMouse.DisplayLayout.Dimensions;

namespace LittleBigMouse.DisplayLayout.Tests
{
    public class UnitTest1
    {
        [Fact]
        public void Test1()
        {
            var s = new DisplaySizeInMm
            {
                Width = 100,
                Height = 50,
                TopBorder = 1,
                BottomBorder = 2,
                LeftBorder = 3,
                RightBorder = 4,
            };

            Assert.Equal(100, s.Bounds.Right);
            Assert.Equal(50, s.Bounds.Height);

            Assert.Equal(53, s.OutsideHeight);
            Assert.Equal(107, s.OutsideWidth);

            Assert.Equal(53, s.OutsideBounds.Height);
            Assert.Equal(107, s.OutsideBounds.Width);



        }
        
        [Fact]
        public void Test2()
        {
            var s = new DisplaySizeInMm()
            {
            };

            Assert.Equal(40, s.OutsideHeight);
            Assert.Equal(40, s.OutsideWidth);

            Assert.Equal(40, s.OutsideBounds.Height);
            Assert.Equal(40, s.OutsideBounds.Width);

            Assert.Equal(0, s.Bounds.Right);
            Assert.Equal(0, s.Bounds.Height);
        }

        [Fact]
        public void TestTranslate()
        {
            var s = new DisplaySizeInMm
            {
                X=10,
                Y=12,
                Width = 100,
                Height = 50,
                TopBorder = 1,
                BottomBorder = 2,
                LeftBorder = 3,
                RightBorder = 4,
            };

            var t = s.Translate(new Vector(1, 2));

            Assert.Equal(11, t.X);
            Assert.Equal(14, t.Y);
        }
    }
}