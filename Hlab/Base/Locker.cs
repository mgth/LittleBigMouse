using System;

namespace Hlab.Base
{
    public class Token
    {
        private readonly object _locked = new object();
        private int _count = 0;

        public Token(int nb = 1)
        {
            _count = nb;
        }

        public TokenGetter TryGet(int nb = 1)
        {
            lock (_locked)
            {
                if (nb <= _count)
                {
                    _count -= nb;
                    return new TokenGetter(this,nb);
                }
                return null;
            }
        }
        public TokenGetter Get(int nb = 1)
        {
            TokenGetter r = null;
            while (r == null) r = TryGet(nb);
            return r;
        }

        public void Add(int nb = 1)
        {
            lock (_locked)
            {
                _count += nb;
            }
        }
    }

    public class TokenGetter : IDisposable
    {
        private readonly Token _token;
        private readonly int _nb;
        public TokenGetter(Token t, int nb)
        {
            _nb = nb;
            _token = t;
        }
        public void Dispose()
        {
            _token.Add(_nb);
        }
    }
}
