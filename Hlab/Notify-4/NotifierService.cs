using Hlab.Base;

namespace Hlab.Notify
{
    public class NotifierService : Singleton<NotifierService>
    {
        protected NotifierService()
        {
            Factory.Register(typeof(object),o=>new Notifier());
        }

        public Factory<Notifier> Factory { get; } = new Factory<Notifier>();

        public Notifier GetNotifier( /*INotifyPropertyChanged*/ object target) => Factory.Get(target);
    }
}
